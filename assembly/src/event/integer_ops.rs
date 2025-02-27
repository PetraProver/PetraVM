use crate::{
    emulator::{Interpreter, InterpreterChannels, InterpreterTables},
    event::Event,
};

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct Add64Event {
    timestamp: u16,
    output: u64,
    input1: u64,
    input2: u64,
    cout: u64,
}

impl Add64Event {
    pub fn new(timestamp: u16, output: u64, input1: u64, input2: u64, cout: u64) -> Self {
        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, input1: u64, input2: u64) -> Self {
        let (output, carry) = input1.overflowing_add(input2);

        let cout = (output ^ input1 ^ input2) >> 1 + (carry as u64) << 63;

        let timestamp = interpreter.timestamp;

        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }
}

impl Event for Add64Event {
    fn fire(&self, _channels: &mut InterpreterChannels, _tables: &InterpreterTables) {
        // No interaction with the state channel.
    }
}

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct Add32Event {
    timestamp: u16,
    output: u32,
    input1: u32,
    input2: u32,
    cout: u32,
}

impl Add32Event {
    pub fn new(timestamp: u16, output: u32, input1: u32, input2: u32, cout: u32) -> Self {
        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, input1: u32, input2: u32) -> Self {
        let (output, carry) = input1.overflowing_add(input2);

        let cout = (output ^ input1 ^ input2) >> 1 + (carry as u32) << 31;

        let timestamp = interpreter.timestamp;

        Self {
            timestamp,
            output,
            input1,
            input2,
            cout,
        }
    }
}

impl Event for Add32Event {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        // No interaction with the state channel.
    }
}

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct AddiEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    dst: u32,
    dst_val: u32,
    src: u32,
    pub(crate) src_val: u32,
    imm: u32,
}

impl AddiEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        dst: u32,
        dst_val: u32,
        src: u32,
        src_val: u32,
        imm: u32,
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, dst: u32, src: u32, imm: u32) -> Self {
        let fp = interpreter.fp;
        let src_val = interpreter.vrom.get(interpreter.fp as usize + src as usize);
        // The following addition is checked thanks to the ADD32 table.
        let dst_val = src_val + imm;
        interpreter
            .vrom
            .set(interpreter.fp as usize + dst as usize, dst_val);

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.pc += 1;

        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
        }
    }
}

impl Event for AddiEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}

// Struture of an event for ADDI.
#[derive(Debug, Clone)]
pub(crate) struct MuliEvent {
    pc: u16,
    fp: u16,
    timestamp: u16,
    dst: u32,
    dst_val: u32,
    src: u32,
    pub(crate) src_val: u32,
    imm: u32,
    // Auxiliary commitments
    pub(crate) aux: [u32; 8],
    // Intermediary sum, such that interm_sum[i] = aux[2*i] + aux[2*i+1], for i > 0.
    // Note: we don't need the initial value because it is equal to sum[0].
    pub(crate) interm_sum: [u64; 3],
    // Sums such that: sum[i] = sum[i-1] + interm_sum[i].
    // Note: we don't need the fourth sum value because it is equal to DST_VAL.
    pub(crate) sum: [u64; 3],
}

impl MuliEvent {
    pub fn new(
        pc: u16,
        fp: u16,
        timestamp: u16,
        dst: u32,
        dst_val: u32,
        src: u32,
        src_val: u32,
        imm: u32,
        aux: [u32; 8],
        interm_sum: [u64; 3],
        sum: [u64; 3],
    ) -> Self {
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            aux,
            interm_sum,
            sum,
        }
    }

    pub fn generate_event(interpreter: &mut Interpreter, dst: u32, src: u32, imm: u32) -> Self {
        let fp = interpreter.fp;
        let src_val = interpreter.vrom.get(interpreter.fp as usize + src as usize);

        let dst_val = src_val * imm;

        interpreter
            .vrom
            .set(interpreter.fp as usize + dst as usize, dst_val);

        let xs = [
            src_val as u8,
            (src_val >> 8) as u8,
            (src_val >> 16) as u8,
            (src_val >> 24) as u8,
        ];
        let ys = [
            imm as u8,
            (imm >> 8) as u8,
            (imm >> 16) as u8,
            (imm >> 24) as u8,
        ];

        let mut aux = [0; 8];
        for i in 0..4 {
            aux[2 * i] = ys[i] as u32 * xs[0] as u32 + (1 << 16) * ys[i] as u32 * xs[2] as u32;
            aux[2 * i + 1] = ys[i] as u32 * xs[1] as u32 + (1 << 16) * ys[i] as u32 * xs[3] as u32;
        }

        // We call the ADD64 gadget to check these additions.
        let mut interm_sum = [0; 3];
        let mut sum = [0; 3];
        sum[0] = aux[0] as u64 + aux[1] as u64;
        for i in 1..3 {
            interm_sum[i - 1] = aux[2 * i] as u64 + aux[2 * i + 1] as u64;
            sum[i] = sum[i - 1] + interm_sum[i - 1];
        }
        interm_sum[2] = aux[6] as u64 + aux[7] as u64;

        let pc = interpreter.pc;
        let timestamp = interpreter.timestamp;
        interpreter.pc += 1;
        Self {
            pc,
            fp,
            timestamp,
            dst,
            dst_val,
            src,
            src_val,
            imm,
            aux,
            interm_sum,
            sum,
        }
    }
}

impl Event for MuliEvent {
    fn fire(&self, channels: &mut InterpreterChannels, tables: &InterpreterTables) {
        channels
            .state_channel
            .pull((self.pc, self.fp, self.timestamp));
        channels
            .state_channel
            .push((self.pc + 1, self.fp, self.timestamp + 1));
    }
}
