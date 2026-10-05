//! What the editor needs from the plugin, as a trait, so the whole UI can run headless
//! in tests against [`MockHost`].

use crate::params::ProgramState;
use crate::{GuiNote, Rs92Params, UiBridge};
use nih_plug::prelude::{ParamPtr, ParamSetter, Params};
use rs92_dsp::Patch;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub trait ParamHost {
    /// Current normalized value.
    fn norm(&self, id: &str) -> f32;
    fn default_norm(&self, id: &str) -> f32;
    /// Number of steps for discrete parameters.
    fn steps(&self, id: &str) -> Option<usize>;
    fn name(&self, id: &str) -> String;
    /// Value as text, with unit.
    fn to_text(&self, id: &str, norm: f32) -> String;
    fn parse_text(&self, id: &str, text: &str) -> Option<f32>;
    fn plain_to_norm(&self, id: &str, plain: f32) -> f32;
    fn begin(&self, id: &str);
    fn set(&self, id: &str, norm: f32);
    fn end(&self, id: &str);
    fn patch(&self) -> Patch;
    fn custom_chords(&self) -> [Vec<u8>; 4];
    fn set_custom_chords(&self, chords: [Vec<u8>; 4]);
    fn program(&self) -> ProgramState;
    fn set_program(&self, p: ProgramState);
    fn lock_mask(&self) -> u8;
    fn set_lock_mask(&self, m: u8);
    fn bridge(&self) -> &UiBridge;
    /// Asks the host to resize the editor window (logical pixels).
    fn request_size(&self, width: u32, height: u32);

    /// One complete gesture.
    fn set_gesture(&self, id: &str, norm: f32) {
        self.begin(id);
        self.set(id, norm);
        self.end(id);
    }

    fn send_note(&self, n: GuiNote) {
        self.bridge().send_note(n);
    }

    /// Pushes a whole patch through the parameter setter and stores its chords.
    fn apply_patch(&self, patch: &Patch) {
        for (id, plain) in rs92_presets::ids::plain_values(patch) {
            let n = self.plain_to_norm(id, plain);
            if (self.norm(id) - n).abs() > 1e-7 {
                self.set_gesture(id, n);
            }
        }
        self.set_custom_chords(patch.custom_chords.map(|c| c.intervals().to_vec()));
    }
}

/// Parameter pointers by ID; shared by the real and the mock host.
pub struct ParamTable {
    pub params: Arc<Rs92Params>,
    ptrs: HashMap<String, ParamPtr>,
}

impl ParamTable {
    pub fn new(params: Arc<Rs92Params>) -> Self {
        let ptrs = params
            .param_map()
            .into_iter()
            .map(|(id, p, _)| (id, p))
            .collect();
        ParamTable { params, ptrs }
    }

    pub fn ptr(&self, id: &str) -> ParamPtr {
        *self
            .ptrs
            .get(id)
            .unwrap_or_else(|| panic!("unknown parameter {id}"))
    }

    // SAFETY for the `unsafe` calls below: the parameter struct is kept alive by `params`.
    fn default_norm(&self, id: &str) -> f32 {
        unsafe { self.ptr(id).default_normalized_value() }
    }
    fn steps(&self, id: &str) -> Option<usize> {
        unsafe { self.ptr(id).step_count() }
    }
    fn name(&self, id: &str) -> String {
        unsafe { self.ptr(id).name() }.to_string()
    }
    fn to_text(&self, id: &str, norm: f32) -> String {
        unsafe { self.ptr(id).normalized_value_to_string(norm, true) }
    }
    fn parse_text(&self, id: &str, text: &str) -> Option<f32> {
        unsafe { self.ptr(id).string_to_normalized_value(text) }
    }
    fn plain_to_norm(&self, id: &str, plain: f32) -> f32 {
        unsafe { self.ptr(id).preview_normalized(plain) }
    }
    fn plain(&self, id: &str, norm: f32) -> f32 {
        unsafe { self.ptr(id).preview_plain(norm) }
    }
}

macro_rules! table_forward {
    () => {
        fn default_norm(&self, id: &str) -> f32 {
            self.table.default_norm(id)
        }
        fn steps(&self, id: &str) -> Option<usize> {
            self.table.steps(id)
        }
        fn name(&self, id: &str) -> String {
            self.table.name(id)
        }
        fn to_text(&self, id: &str, norm: f32) -> String {
            self.table.to_text(id, norm)
        }
        fn parse_text(&self, id: &str, text: &str) -> Option<f32> {
            self.table.parse_text(id, text)
        }
        fn plain_to_norm(&self, id: &str, plain: f32) -> f32 {
            self.table.plain_to_norm(id, plain)
        }
    };
}

/// The real host: nih-plug's parameter setter for this frame.
pub struct PluginHost<'a> {
    pub table: &'a ParamTable,
    pub bridge: &'a UiBridge,
    pub setter: &'a ParamSetter<'a>,
}

impl ParamHost for PluginHost<'_> {
    table_forward!();

    fn norm(&self, id: &str) -> f32 {
        unsafe { self.table.ptr(id).unmodulated_normalized_value() }
    }
    fn begin(&self, id: &str) {
        unsafe {
            self.setter
                .raw_context
                .raw_begin_set_parameter(self.table.ptr(id))
        }
    }
    fn set(&self, id: &str, norm: f32) {
        unsafe {
            self.setter
                .raw_context
                .raw_set_parameter_normalized(self.table.ptr(id), norm)
        }
    }
    fn end(&self, id: &str) {
        unsafe {
            self.setter
                .raw_context
                .raw_end_set_parameter(self.table.ptr(id))
        }
    }
    fn patch(&self) -> Patch {
        self.table.params.to_patch(&Patch::default())
    }
    fn custom_chords(&self) -> [Vec<u8>; 4] {
        self.table
            .params
            .custom_chords
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }
    fn set_custom_chords(&self, chords: [Vec<u8>; 4]) {
        if let Ok(mut c) = self.table.params.custom_chords.write() {
            *c = chords;
        }
    }
    fn program(&self) -> ProgramState {
        self.table
            .params
            .program
            .read()
            .map(|p| p.clone())
            .unwrap_or_default()
    }
    fn set_program(&self, p: ProgramState) {
        if let Ok(mut s) = self.table.params.program.write() {
            *s = p;
        }
    }
    fn lock_mask(&self) -> u8 {
        self.table.params.lock_mask.read().map(|m| *m).unwrap_or(0)
    }
    fn set_lock_mask(&self, m: u8) {
        if let Ok(mut s) = self.table.params.lock_mask.write() {
            *s = m;
        }
    }
    fn bridge(&self) -> &UiBridge {
        self.bridge
    }
    fn request_size(&self, width: u32, height: u32) {
        self.table
            .params
            .editor_state
            .set_requested_size((width, height));
    }
}

/// In-memory host for headless UI tests. Records every gesture.
pub struct MockHost {
    pub table: ParamTable,
    pub values: RefCell<HashMap<String, f32>>,
    pub gestures: RefCell<Vec<(String, &'static str)>>,
    pub chords: RefCell<[Vec<u8>; 4]>,
    pub program: RefCell<ProgramState>,
    pub locks: RefCell<u8>,
    pub bridge: Arc<UiBridge>,
    pub notes: RefCell<rtrb::Consumer<GuiNote>>,
    /// Window sizes the UI asked for, oldest first.
    pub size_requests: RefCell<Vec<(u32, u32)>>,
    _scope: rtrb::Producer<f32>,
}

impl Default for MockHost {
    fn default() -> Self {
        let table = ParamTable::new(Arc::new(Rs92Params::default()));
        let values = table
            .ptrs
            .keys()
            .map(|id| (id.clone(), table.default_norm(id)))
            .collect();
        let (bridge, scope, notes) = UiBridge::new();
        MockHost {
            table,
            values: RefCell::new(values),
            gestures: RefCell::new(vec![]),
            chords: RefCell::new(Default::default()),
            program: RefCell::new(ProgramState {
                preset_name: "LANDLORD '91".into(),
                bank: "A".into(),
                ..Default::default()
            }),
            locks: RefCell::new(0),
            bridge,
            notes: RefCell::new(notes),
            size_requests: RefCell::new(vec![]),
            _scope: scope,
        }
    }
}

impl MockHost {
    /// Drains the notes the UI sent to the "audio thread".
    pub fn take_notes(&self) -> Vec<GuiNote> {
        let mut out = vec![];
        while let Ok(n) = self.notes.borrow_mut().pop() {
            out.push(n);
        }
        out
    }

    pub fn plain(&self, id: &str) -> f32 {
        self.table.plain(id, self.norm(id))
    }
}

impl ParamHost for MockHost {
    table_forward!();

    fn norm(&self, id: &str) -> f32 {
        self.values.borrow()[id]
    }
    fn begin(&self, id: &str) {
        self.gestures.borrow_mut().push((id.to_string(), "begin"));
    }
    fn set(&self, id: &str, norm: f32) {
        // Mirror the host: store the snapped normalized value.
        let snapped = self
            .table
            .plain_to_norm(id, self.table.plain(id, norm.clamp(0.0, 1.0)));
        self.values.borrow_mut().insert(id.to_string(), snapped);
        self.gestures.borrow_mut().push((id.to_string(), "set"));
    }
    fn end(&self, id: &str) {
        self.gestures.borrow_mut().push((id.to_string(), "end"));
    }
    fn patch(&self) -> Patch {
        let mut p = Patch::default();
        for d in rs92_presets::ids::all() {
            rs92_presets::ids::set_plain(&mut p, d.id, self.plain(d.id));
        }
        for (dst, src) in p.custom_chords.iter_mut().zip(self.chords.borrow().iter()) {
            *dst = rs92_presets::preset::chord_from_intervals(src);
        }
        p
    }
    fn custom_chords(&self) -> [Vec<u8>; 4] {
        self.chords.borrow().clone()
    }
    fn set_custom_chords(&self, chords: [Vec<u8>; 4]) {
        *self.chords.borrow_mut() = chords;
    }
    fn program(&self) -> ProgramState {
        self.program.borrow().clone()
    }
    fn set_program(&self, p: ProgramState) {
        *self.program.borrow_mut() = p;
    }
    fn lock_mask(&self) -> u8 {
        *self.locks.borrow()
    }
    fn set_lock_mask(&self, m: u8) {
        *self.locks.borrow_mut() = m;
    }
    fn bridge(&self) -> &UiBridge {
        &self.bridge
    }
    fn request_size(&self, width: u32, height: u32) {
        self.size_requests.borrow_mut().push((width, height));
    }
}
