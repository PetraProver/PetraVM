use std::{
    collections::HashMap,
    ops::{Deref, Index},
    rc::Rc,
};

use binius_field::{BinaryField, BinaryField32b, Field};
use zcrayvm_assembly::{
    memory::vrom_allocator::VromAllocator, Assembler, Memory, ValueRom, ZCrayTrace,
};

pub struct TestFrameHandle {
    frames_ref: Rc<DefinedFrame>,
}

impl Deref for TestFrameHandle {
    type Target = DefinedFrame;

    fn deref(&self) -> &Self::Target {
        &self.frames_ref
    }
}

/// Type that holds all frames that are expected to be constructed during a
/// test.
///
/// `FrameTemplate`s that are added through `add_frame` will be hydrated into
/// `DefinedFrame`s (which have their own frame VROM slice). The idea is at the
/// start, the test writer defines the frames in the order that they are
/// expected to be constructed through a run of the program.
pub struct Frames {
    frames: HashMap<&'static str, Vec<Rc<DefinedFrame>>>,
    trace: Rc<ZCrayTrace>,

    /// When writing a test, we know the size of the frames that should be
    /// allocated. However, we do not know (and also should not know) the
    /// strategy for choosing blocks of memory to allocate. As such, we're going
    /// to keep a "mock" allocator just to know that if allocation calls are
    /// made in a deterministic order, what VROM addresses are picked for each
    /// allocation. This also allows the allocator logic to change without
    /// breaking any of the tests.
    mock_allocator: VromAllocator,
}

impl Frames {
    pub fn new(trace: Rc<ZCrayTrace>) -> Self {
        Self {
            frames: HashMap::new(),
            trace,
            mock_allocator: VromAllocator::default(),
        }
    }

    pub fn add_frame(&mut self, frame_temp: &FrameTemplate) -> TestFrameHandle {
        let start_addr = self.mock_allocator.alloc(frame_temp.frame_size);
        let frame = Rc::new(frame_temp.clone().build(self.trace.clone(), start_addr));

        let label_frames = self.frames.entry(frame.label).or_default();
        label_frames.push(frame.clone());

        TestFrameHandle { frames_ref: frame }
    }
}

impl Index<&'static str> for Frames {
    type Output = [Rc<DefinedFrame>];

    fn index(&self, index: &'static str) -> &Self::Output {
        &self.frames[index]
    }
}

#[derive(Clone)]
pub(crate) struct FrameTemplate {
    label: &'static str,
    frame_size: u32,
}

impl FrameTemplate {
    pub fn new(label: &'static str, frame_size: u32) -> Self {
        Self { label, frame_size }
    }

    pub fn build(self, trace: Rc<ZCrayTrace>, frame_start_addr: u32) -> DefinedFrame {
        DefinedFrame {
            label: self.label,
            trace,
            frame_start_addr,
            frame_size: self.frame_size,
        }
    }
}

/// A frame that has been created from a template.
///
/// Unlike a template, a `DefinedFrame` has it's own range of VROM and can be
/// queried directly to get a values within the frame. The idea behind this is
/// to simplify accessing frames a bit more.
pub struct DefinedFrame {
    label: &'static str,
    trace: Rc<ZCrayTrace>,
    frame_start_addr: u32,
    frame_size: u32,
}

impl DefinedFrame {
    pub fn get_vrom_u32_expected(&self, frame_slot: u32) -> u32 {
        assert!(
            frame_slot <= self.frame_size,
            "Attempted to access a frame slot outside of the frame (Frame: {}[{}] (size: {}))",
            self.label,
            self.frame_start_addr,
            self.frame_start_addr
        );

        let slot_addr = self.frame_start_addr + frame_slot;
        self.trace.vrom().read::<u32>(slot_addr).expect("")
    }
}

// This can be reused between tests.
pub fn generate_trace_and_validate(asm_bytes: &str, init_vals: &[u32]) -> Rc<ZCrayTrace> {
    // Use the multiplicative generator G for calculations
    const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;

    let compiled_program = Assembler::from_code(asm_bytes).unwrap();

    let mut processed_init_vals = Vec::with_capacity(2 + init_vals.len());
    processed_init_vals.extend([0, 0]);
    processed_init_vals.extend(init_vals.iter().map(|x| G.pow([*x as u64]).val()));

    println!("PROC: {:?}", processed_init_vals);

    println!("Before new init");

    let vrom = ValueRom::new_with_init_vals(&processed_init_vals);

    println!("Before Memory");

    let memory = Memory::new(compiled_program.prom, vrom);

    println!("Before Trace");

    // Execute the program and generate the trace
    let (trace, boundary_values) = ZCrayTrace::generate(
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .expect("Trace generation should not fail");

    println!("Before validate");

    // Validate the trace
    trace.validate(boundary_values);

    Rc::new(trace)
}
