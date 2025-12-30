# ---- Connect to QEMU's GDB server ----
target remote localhost:1234

# ---- Custom commands ----
define mysi
  si
  x/5i $pc
end

define regs
  info registers
  info registers eflags
end

# ---- Quick view setup (can be run manually) ----
define init
  b *(mboot_entry - 0xFFFFFFFF80000000)
  c
end
