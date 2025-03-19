#[framesize(0x10)]
_start:
  LDI.W @2, #0
  CALLI test_binary_field, @3
  LDI.W @4, #0
  RET

#[framesize(0x30)]
test_binary_field:
  LDI.W @2, #0
  RET
