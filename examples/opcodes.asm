#[framesize(0x10)]
_start:
   CALLI test_binary_field, @3
  RET

#[framesize(0x30)]
test_binary_field:
  RET
