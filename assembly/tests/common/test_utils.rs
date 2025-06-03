use std::{
    any::type_name,
    collections::HashMap,
    ops::{Deref, Index},
    rc::Rc,
};

use petravm_asm::{
    isa::GenericISA,
    memory::{vrom::VromValueT, vrom_allocator::VromAllocator},
    AssembledProgram, Assembler, Memory, PetraTrace, ValueRom,
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
#[derive(Clone, Debug)]
pub struct Frames {
    /// Map of frame templates names to instantiated frames of that template (in
    /// order of allocation).
    frames: HashMap<String, Vec<Rc<AllocatedFrame>>>,
    pub trace: Rc<PetraTrace>,

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
    fn new(trace: Rc<PetraTrace>, frame_templates: HashMap<String, FrameTemplate>) -> Self {
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
    pub fn build(self, trace: Rc<PetraTrace>, frame_start_addr: u32) -> AllocatedFrame {
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
    trace: Rc<PetraTrace>,
    frame_start_addr: u32,
    frame_size: u32,
}

impl AllocatedFrame {
    pub fn get_vrom_expected<T: VromValueT>(&self, frame_slot: u32) -> T {
        // If `T` is multiple words long, then we actually read more than the slot at
        // `frame_slot`.
        let highest_slot_accessed = (T::word_size() as u32 - 1) + frame_slot;

        assert!(
            highest_slot_accessed <= self.frame_size,
            "Attempted to access a frame slot outside of the frame (Frame: {}[{}] (frame_size: {}, read_size: {}))",
            self.label,
            self.frame_start_addr,
            self.frame_size,
            T::word_size(),
        );

        let vrom_addr = self.frame_start_addr + frame_slot;
        self.trace
            .vrom()
            .read(vrom_addr)
            .unwrap_or_else(|_| Self::vrom_read_err_panic(type_name::<T>()))
    }

    fn vrom_read_err_panic(read_size_str: &str) -> ! {
        panic!("Reading a {read_size_str} from VROM memory that is expected to be filled")
    }
}

/// Create frame templates from all labels annotated with `#[framesize(0x*)]`.
fn extract_frame_templates_from_assembled_program(
    assembled_program: &AssembledProgram,
) -> HashMap<String, FrameTemplate> {
    let mut frame_templates = HashMap::new();

    for (label, (pc_location, _, _)) in assembled_program.labels.iter() {
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

/// Binary(ies) to execute along with initial stack values.
#[derive(Debug)]
pub struct AsmToExecute {
    asm_bytes: String,

    /// Note that the first two values on the stack are always both set to `0`.
    /// These values are applied to stack values [..2] onwards.
    init_vals: Vec<u32>,
}

impl AsmToExecute {
    pub fn new(asm_bytes: &str) -> Self {
        Self {
            asm_bytes: asm_bytes.to_string(),
            init_vals: Vec::default(),
        }
    }

    /// Add an additional binary to include.
    ///
    /// Note that program execution always starts at the first instruction in
    /// the first binary. Also note that subsequent binaries are just
    /// concatenated together.
    pub fn add_binary(mut self, asm_bytes: &str) -> Self {
        self.asm_bytes.push_str(asm_bytes);
        self
    }

    pub fn init_vals(mut self, init_vals: Vec<u32>) -> Self {
        self.init_vals = init_vals;
        self
    }
}

impl From<&'static str> for AsmToExecute {
    fn from(v: &'static str) -> Self {
        Self::new(v)
    }
}

#[derive(Clone, Debug)]
pub struct ExecutedTestProgInfo {
    pub frames: Frames,
    pub compiled_program: AssembledProgram,
}

/// Common logic that all ASM tests need to run.
///
/// Note that `init_vals` are converted to a 32-bit binary field.
pub fn execute_test_asm<T: Into<AsmToExecute>>(prog: T) -> ExecutedTestProgInfo {
    let prog: AsmToExecute = prog.into();

    // Init the tracing subscriber if not already initialized.
    let _ = tracing_subscriber::fmt::try_init();

    let compiled_program = Assembler::from_code(&prog.asm_bytes).unwrap();
    let frame_templates = extract_frame_templates_from_assembled_program(&compiled_program);

    let mut processed_init_vals = Vec::with_capacity(2 + prog.init_vals.len());

    // We always start execution on PC = 0, so the initial VROM should always
    // contain [0, 0].
    processed_init_vals.extend([0, 0]);
    processed_init_vals.extend(prog.init_vals);

    let vrom = ValueRom::new_with_init_vals(&processed_init_vals);
    let memory = Memory::new(compiled_program.prom.clone(), vrom);

    // Execute the program and generate the trace
    let (trace, boundary_values) = PetraTrace::generate(
        Box::new(GenericISA),
        memory,
        compiled_program.frame_sizes.clone(),
        compiled_program.pc_field_to_index_pc.clone(),
    )
    .expect("Trace generation should not fail");

    // Validate the trace
    trace.validate(boundary_values);

    ExecutedTestProgInfo {
        frames: Frames::new(Rc::new(trace), frame_templates),
        compiled_program,
    }
}
