use std::{
    collections::HashMap,
    ops::{Deref, Index},
    rc::Rc,
};

use binius_field::{BinaryField, BinaryField32b, Field};
use zcrayvm_assembly::{
    isa::GenericISA, memory::vrom_allocator::VromAllocator, AssembledProgram, Assembler, Memory,
    ValueRom, ZCrayTrace,
};

// Lightweight handle that can be dereferenced to the actual frame.
pub struct TestFrameHandle {
    frames_ref: Rc<AllocatedFrame>,
}

impl Deref for TestFrameHandle {
    type Target = AllocatedFrame;

    fn deref(&self) -> &Self::Target {
        &self.frames_ref
    }
}

/// Type that holds all frames that are expected to be constructed during a
/// test.
///
/// `FrameTemplate`s that are added through `add_frame` will be hydrated into
/// `AllocatedFrame`s (which have their own frame VROM slice). The idea is at
/// the start, the test writer defines the frames in the order that they are
/// expected to be constructed through a run of the program.
#[derive(Debug)]
pub struct Frames {
    /// Map of frame templates names to instantiated frames of that template (in
    /// order of allocation).
    frames: HashMap<String, Vec<Rc<AllocatedFrame>>>,
    trace: Rc<ZCrayTrace>,

    frame_templates: HashMap<String, FrameTemplate>,

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
    pub fn new(trace: Rc<ZCrayTrace>, frame_templates: HashMap<String, FrameTemplate>) -> Self {
        Self {
            frames: HashMap::new(),
            trace,
            frame_templates,
            mock_allocator: VromAllocator::default(),
        }
    }

    pub fn add_frame(&mut self, frame_name: &str) -> TestFrameHandle {
        let frame_temp = self
            .frame_templates
            .get(frame_name)
            .cloned()
            .expect("Frame template should exist");
        self.add_frame_intern(frame_temp)
    }

    fn add_frame_intern(&mut self, frame_temp: FrameTemplate) -> TestFrameHandle {
        let start_addr = self.mock_allocator.alloc(frame_temp.frame_size);
        let frame = Rc::new(frame_temp.build(self.trace.clone(), start_addr));

        let label_frames = self.frames.entry(frame.label.clone()).or_default();
        label_frames.push(frame.clone());

        TestFrameHandle { frames_ref: frame }
    }
}

impl Index<&'static str> for Frames {
    type Output = [Rc<AllocatedFrame>];

    fn index(&self, index: &'static str) -> &Self::Output {
        &self.frames[index]
    }
}

/// Information describing a frame that will be constructed by a `CALL*` or
/// `TAIL` call. Templates are used to instantiate actual frames during a test.
#[derive(Clone, Debug)]
pub(crate) struct FrameTemplate {
    label: String,

    // Technically this can only be a `u16`, but upcasting it to a `u32` lets us avoid casts in a
    // bunch of other areas.
    frame_size: u32,
}

impl FrameTemplate {
    pub fn build(self, trace: Rc<ZCrayTrace>, frame_start_addr: u32) -> AllocatedFrame {
        AllocatedFrame {
            label: self.label,
            trace,
            frame_start_addr,
            frame_size: self.frame_size,
        }
    }
}

#[derive(Debug)]
/// A frame that has been created from a template.
///
/// Unlike a template, a `AllocatedFrame` has it's own range of VROM addresses
/// and can be queried directly to get a values within the frame. The idea
/// behind this is to avoid having to calculate VROM addresses accessed during a
/// test "by hand" and instead only worry about slot offsets from a given frame.
pub struct AllocatedFrame {
    label: String,
    trace: Rc<ZCrayTrace>,
    frame_start_addr: u32,
    frame_size: u32,
}

impl AllocatedFrame {
    pub fn get_vrom_u32_expected(&self, frame_slot: u32) -> u32 {
        assert!(
            frame_slot <= self.frame_size,
            "Attempted to access a frame slot outside of the frame (Frame: {}[{}] (size: {}))",
            self.label,
            self.frame_start_addr,
            self.frame_size
        );

        let slot_addr = self.frame_start_addr + frame_slot;
        self.trace.vrom().read::<u32>(slot_addr).expect("")
    }
}

/// Create frame templates from all labels annotated with `#[framesize(0x*)]`.
fn extract_frame_templates_from_assembled_program(
    assembled_program: &AssembledProgram,
) -> HashMap<String, FrameTemplate> {
    let mut frame_templates = HashMap::new();

    for (label, pc_location) in assembled_program.labels.iter() {
        // If the label has a frame size, then create a template for it.
        if let Some(frame_size) = assembled_program.frame_sizes.get(pc_location) {
            let frame_temp = FrameTemplate {
                label: label.clone(),
                frame_size: *frame_size as u32,
            };

            frame_templates.insert(label.clone(), frame_temp);
        }
    }

    frame_templates
}

/// Common logic that all ASM tests need to run.
///
/// Note that `init_vals` are converted to a 32-bit binary field.
pub fn execute_test_asm(asm_bytes: &str, init_vals: &[u32]) -> Frames {
    // Use the multiplicative generator G for calculations
    const G: BinaryField32b = BinaryField32b::MULTIPLICATIVE_GENERATOR;

    let compiled_program = Assembler::from_code(asm_bytes).unwrap();
    let frame_templates = extract_frame_templates_from_assembled_program(&compiled_program);

    let mut processed_init_vals = Vec::with_capacity(2 + init_vals.len());

    // We always start execution on PC = 0, so the initial VROM should always
    // contain [0, 0].
    processed_init_vals.extend([0, 0]);
    processed_init_vals.extend(init_vals.iter().map(|x| G.pow([*x as u64]).val()));

    let vrom = ValueRom::new_with_init_vals(&processed_init_vals);
    let memory = Memory::new(compiled_program.prom, vrom);

    // Execute the program and generate the trace
    let (trace, boundary_values) = ZCrayTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes,
        compiled_program.pc_field_to_int,
    )
    .expect("Trace generation should not fail");

    // Validate the trace
    trace.validate(boundary_values);

    Frames::new(Rc::new(trace), frame_templates)
}
