ENTRY(_start)
MEMORY
{
    ROM (rx)  : ORIGIN = 0x00100000, LENGTH = 1M
    RAM (rwx) : ORIGIN = 0x00200000, LENGTH = 68M
}
SECTIONS
{
    .text : AT(0x00100000)
    {
        KEEP(*(.text.prologue))
        *(.text)
        *(.text*)
    } > ROM

    .rodata : ALIGN(4096) { *(.rodata*) } > ROM
    .data   : ALIGN(4096) { *(.data*) } > RAM

    .bss : ALIGN(4096)
    {
        *(COMMON)
        *(.bss*)
        . = ALIGN(64);
        _token_lane_start = .;
        . += 0x3000000; /* Statically allocate exactly 48MB inside BSS */
        _token_lane_end = .;
    } > RAM

    /DISCARD/ : { *(.comment) *(.eh_frame) }
}
