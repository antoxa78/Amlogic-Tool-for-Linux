/* LD_PRELOAD shim: after USB enumeration, rewrite idProduct 0xc004 -> 0xc003
 * by scanning process memory for the Amlogic VID/PID pattern.
 *
 * Build: gcc -shared -fPIC -o dnl_shim.so dnl_shim.c -ldl
 * Use:   LD_PRELOAD=./dnl_shim.so ./update ...
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <sys/mman.h>

typedef int (*orig_find_devices_t)(void);
static int patched = 0;

/* Scan readable memory for the pattern 1b8e c004 (LE) and change to 1b8e c003 */
static void scan_and_patch(void) {
    if (patched) return;
    FILE *maps = fopen("/proc/self/maps", "r");
    if (!maps) return;

    char line[512];
    while (fgets(line, sizeof(line), maps)) {
        unsigned long start, end;
        char perms[8];
        if (sscanf(line, "%lx-%lx %4s", &start, &end, perms) != 3) continue;
        /* Only scan rw memory (heap, data, etc.) */
        if (perms[0] != 'r' || perms[1] != 'w') continue;
        /* Skip stack and huge regions */
        if (end - start > 16UL * 1024 * 1024) continue;

        size_t len = end - start;
        unsigned char *addr = (unsigned char *)start;

        /* Try to read this region */
        unsigned char *buf = malloc(len);
        if (!buf) continue;
        ssize_t copied = pread(0, buf, len, (off_t)start); /* doesn't work */
        free(buf);

        /* Use /proc/self/mem to read */
        int mem_fd = -1;
        /* Better: try accessing via mmap if rw */
        /* Actually let's use a simpler approach: follow the usb_busses pointer */
    }
    fclose(maps);

    /* ---- Alternative: follow usb_busses directly ---- */
    /* The usb_busses symbol is exported by libusb-0.1 */
    extern struct usb_bus { struct usb_bus *next; } *usb_busses;

    if (!usb_busses) {
        fprintf(stderr, "[dnl_shim] usb_busses is NULL\n");
        return;
    }

    fprintf(stderr, "[dnl_shim] walking usb_busses chain...\n");

    for (struct usb_bus *bus = usb_busses; bus; bus = bus->next) {
        /* Scan 8KB around the bus struct for 1b8e pattern.
         * usb_device structs are allocated nearby or at least the
         * descriptor will be in the same general memory region. */
        unsigned char *base = (unsigned char *)bus;
        for (size_t off = 0; off < 65536; off += 2) {
            /* Check for segfault by using /proc/self/mem read */
            uint16_t val;
            ssize_t r = pread(mem_fd, &val, 2, (off_t)(base + off));
            if (r != 2) break;

            if (val != 0x1b8e) continue;

            r = pread(mem_fd, &val, 2, (off_t)(base + off + 2));
            if (r != 2) break;

            if (val == 0xc004) {
                /* Found it — write 0xc003 */
                uint16_t new_val = 0xc003;
                ssize_t w = pwrite(mem_fd, &new_val, 2, (off_t)(base + off + 2));
                if (w == 2) {
                    fprintf(stderr, "[dnl_shim] *** PATCHED PID 0xc004 -> 0xc003 at +0x%zx ***\n", off);
                    patched = 1;
                    close(mem_fd);
                    return;
                }
            } else if (val == 0xc003) {
                fprintf(stderr, "[dnl_shim] PID already 0xc003 at +0x%zx\n", off);
                patched = 1;
                close(mem_fd);
                return;
            }
        }
    }

    close(mem_fd);

    if (!patched)
        fprintf(stderr, "[dnl_shim] no Amlogic device with PID 0xc004 found\n");
}

int usb_find_devices(void) {
    orig_find_devices_t orig = (orig_find_devices_t)dlsym(RTLD_NEXT, "usb_find_devices");
    int ret = orig ? orig() : -1;
    scan_and_patch();
    return ret;
}

__attribute__((constructor))
static void init(void) {
    fprintf(stderr, "[dnl_shim] loaded (pid=%d)\n", getpid());
}
