//! The Download scene's tree view, mirroring `pacenotes_installer::tree::Tree`;
//! each item's `lParam` is its node index in the model.

use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::Controls as ctl;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

const TVM_INSERTITEMW: u32 = 0x1132;
const TVM_DELETEITEM: u32 = 0x1101;
const TVM_SETITEMW: u32 = 0x113F;
const TVM_SETEXTENDEDSTYLE: u32 = 0x112C;
const TVI_ROOT: isize = -0x10000;
const TVI_LAST: isize = -0x0FFFE;
/// `TVN_FIRST - 17`, which windows-sys does not declare.
pub const TVN_ITEMCHANGEDW: u32 = (-417i32) as u32;

/// The state image lives in bits 12-15: 1 unchecked, 2 checked, 3 partial.
const STATE_IMAGE_SHIFT: u32 = 12;
const STATE_IMAGE_UNCHECKED: u32 = 1;
const STATE_IMAGE_CHECKED: u32 = 2;
const STATE_IMAGE_PARTIAL: u32 = 3;

/// `NMTVITEMCHANGE`, which windows-sys does not declare.
#[repr(C)]
struct NmTvItemChange {
    header: ctl::NMHDR,
    change: u32,
    item: *mut core::ffi::c_void,
    state_new: u32,
    state_old: u32,
    lparam: LPARAM,
}

/// Partial checkboxes are an extended style, which a template cannot request.
pub unsafe fn enable_partial_checkboxes(dialog: HWND) {
    wm::SendMessageW(
        tree_control(dialog),
        TVM_SETEXTENDEDSTYLE,
        ctl::TVS_EX_PARTIALCHECKBOXES as WPARAM,
        ctl::TVS_EX_PARTIALCHECKBOXES as LPARAM,
    );
}

/// Replaces the control's items with the model's.
pub unsafe fn populate(dialog: HWND, state: &mut crate::wizard::state::WizardState) {
    let control = tree_control(dialog);
    state.tree_syncing = true;
    wm::SendMessageW(control, TVM_DELETEITEM, 0, TVI_ROOT);
    state.tree_items = vec![0isize; state.tree.len()];

    for root in state.tree.roots().to_vec() {
        insert_node(control, state, root, TVI_ROOT);
    }
    refresh_checks(dialog, state);
    state.tree_syncing = false;
}

/// Grouping rows start expanded, so the catalogue is visible at a glance.
unsafe fn insert_node(
    control: HWND,
    state: &mut crate::wizard::state::WizardState,
    node: usize,
    parent: isize,
) {
    let text = crate::i18n::to_wide(&row_label(state, node));
    let mut item: ctl::TVINSERTSTRUCTW = core::mem::zeroed();
    item.hParent = parent;
    item.hInsertAfter = TVI_LAST;
    item.Anonymous.itemex.mask = ctl::TVIF_TEXT | ctl::TVIF_PARAM | ctl::TVIF_STATE;
    item.Anonymous.itemex.pszText = text.as_ptr() as *mut u16;
    item.Anonymous.itemex.lParam = node as LPARAM;
    item.Anonymous.itemex.stateMask = ctl::TVIS_EXPANDED;
    item.Anonymous.itemex.state = if state.tree.node(node).is_leaf() {
        0
    } else {
        ctl::TVIS_EXPANDED
    };

    let handle = wm::SendMessageW(control, TVM_INSERTITEMW, 0, &item as *const _ as LPARAM);
    state.tree_items[node] = handle;
    for child in state.tree.children(node).to_vec() {
        insert_node(control, state, child, handle);
    }
}

/// The model's label plus the localized state suffix the model leaves out.
fn row_label(state: &crate::wizard::state::WizardState, node: usize) -> String {
    let node = state.tree.node(node);
    let suffix = match node.state {
        None | Some(pacenotes_installer::tree::LeafState::NotCached) => return node.label.clone(),
        Some(pacenotes_installer::tree::LeafState::Downloaded) => crate::i18n::IDS_STATE_DOWNLOADED,
        Some(pacenotes_installer::tree::LeafState::UpdateAvailable) => {
            crate::i18n::IDS_STATE_UPDATE
        }
        Some(pacenotes_installer::tree::LeafState::LocalOnly) => crate::i18n::IDS_STATE_LOCAL,
    };
    format!("{} {}", node.label, crate::i18n::load_string(suffix))
}

/// Pushes every node's checkbox from the model into the control.
unsafe fn refresh_checks(dialog: HWND, state: &mut crate::wizard::state::WizardState) {
    let control = tree_control(dialog);
    let was_syncing = state.tree_syncing;
    state.tree_syncing = true;
    for node in 0..state.tree.len() {
        let handle = state.tree_items[node];
        if handle == 0 {
            continue;
        }
        let image = match state.tree.check_of(node) {
            pacenotes_installer::tree::Check::Unchecked => STATE_IMAGE_UNCHECKED,
            pacenotes_installer::tree::Check::Checked => STATE_IMAGE_CHECKED,
            pacenotes_installer::tree::Check::Partial => STATE_IMAGE_PARTIAL,
        };
        let mut item: ctl::TVITEMEXW = core::mem::zeroed();
        item.mask = ctl::TVIF_HANDLE | ctl::TVIF_STATE;
        item.hItem = handle;
        item.stateMask = ctl::TVIS_STATEIMAGEMASK;
        item.state = image << STATE_IMAGE_SHIFT;
        wm::SendMessageW(control, TVM_SETITEMW, 0, &item as *const _ as LPARAM);
    }
    state.tree_syncing = was_syncing;
}

/// Takes a toggled checkbox into the model, which propagates it through the
/// subtree, then pushes the whole model back.
pub unsafe fn on_item_changed(dialog: HWND, lparam: LPARAM) -> isize {
    let Some(state) = crate::wizard::page::state(dialog) else {
        return 0;
    };
    if state.tree_syncing {
        return 0;
    }
    let Some(change) = (lparam as *const NmTvItemChange).as_ref() else {
        return 0;
    };
    let node = change.lparam as usize;
    if node >= state.tree.len() {
        return 0;
    }
    let image = (change.state_new & ctl::TVIS_STATEIMAGEMASK) >> STATE_IMAGE_SHIFT;
    state.tree.set_checked(node, image == STATE_IMAGE_CHECKED);
    refresh_checks(dialog, state);
    0
}

unsafe fn tree_control(dialog: HWND) -> HWND {
    wm::GetDlgItem(dialog, crate::wizard::control_ids::IDC_S4_TREE as i32)
}
