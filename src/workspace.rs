//! A Workspace is a set of displayed clients and a set of Layouts for arranging them
use crate::client::Client;
use crate::data_types::{Change, Direction, Region, ResizeAction, Ring, WinId};
use crate::layout::{Layout, LayoutConf};
use std::collections::HashMap;

/**
 * A Workspace represents a named set of clients that are tiled according
 * to a specific layout. Layout properties are tracked per workspace and
 * clients are referenced by ID. Workspaces are independant of monitors and
 * can be moved between monitors freely, bringing their clients with them.
 *
 * The parent WindowManager struct tracks which client is focused from the
 * point of view of the X server by checking focus at the Workspace level
 * whenever a new Workspace becomes active.
 */
#[derive(Debug)]
pub struct Workspace {
    name: &'static str,
    clients: Ring<WinId>,
    layouts: Ring<Layout>,
}

impl Workspace {
    pub fn new(name: &'static str, layouts: Vec<Layout>) -> Workspace {
        if layouts.len() == 0 {
            panic!("{}: require at least one layout function", name);
        }

        Workspace {
            name,
            clients: Ring::new(Vec::new()),
            layouts: Ring::new(layouts),
        }
    }

    /// The number of clients currently on this workspace
    pub fn len(&self) -> usize {
        self.clients.len()
    }

    /// Iterate over the clients on this workspace in position order
    pub fn iter(&self) -> std::collections::vec_deque::Iter<WinId> {
        self.clients.iter()
    }

    /// A reference to the currently focused client if there is one
    pub fn focused_client(&self) -> Option<WinId> {
        self.clients.focused().map(|c| *c)
    }

    /// Add a new client to this workspace at the top of the stack and focus it
    pub fn add_client(&mut self, id: WinId) {
        self.clients.insert(0, id);
    }

    /// Focus the client with the given id, returns an option of the previously focused
    /// client if there was one
    pub fn focus_client(&mut self, id: WinId) -> Option<WinId> {
        if self.clients.len() == 0 {
            return None;
        }

        let prev = self.clients.focused().unwrap().clone();
        self.clients.focus_by(|c| c == &id);
        Some(prev)
    }

    /// Remove a target client, retaining focus at the same position in the stack.
    /// Returns the removed client if there was one to remove.
    pub fn remove_client(&mut self, id: WinId) -> Option<WinId> {
        self.clients.remove_by(|c| c == &id)
    }

    /// Remove the currently focused client, keeping focus at the same position in the stack.
    /// Returns the removed client if there was one to remove.
    pub fn remove_focused_client(&mut self) -> Option<WinId> {
        self.clients.remove_focused()
    }

    /// Run the current layout function, generating a list of resize actions to be
    /// applied byt the window manager.
    pub fn arrange(
        &self,
        screen_region: &Region,
        client_map: &HashMap<WinId, Client>,
    ) -> Vec<ResizeAction> {
        if self.clients.len() > 0 {
            let layout = self.layouts.focused().unwrap();
            let clients: Vec<&Client> = self
                .clients
                .iter()
                .map(|id| client_map.get(id).unwrap())
                .collect();
            debug!(
                "applying '{}' layout for {} clients on workspace '{}'",
                layout.symbol,
                self.clients.len(),
                self.name
            );
            layout.arrange(&clients, self.focused_client(), screen_region)
        } else {
            vec![]
        }
    }

    /// Cycle through the available layouts on this workspace
    pub fn cycle_layout(&mut self, direction: Direction) -> &str {
        self.layouts.cycle_focus(direction);
        self.layout_symbol()
    }

    /// The symbol of the currently used layout (passed on creation)
    pub fn layout_symbol(&self) -> &str {
        self.layouts.focused().unwrap().symbol
    }

    /**
     * The LayoutConf of the currently active Layout. Used by the WindowManager to
     * determine when and how the layout function should be applied.
     */
    pub fn layout_conf(&self) -> LayoutConf {
        self.layouts.focused().unwrap().conf
    }

    /// Cycle focus through the clients on this workspace
    pub fn cycle_client(&mut self, direction: Direction) -> Option<(WinId, WinId)> {
        if self.clients.len() < 2 {
            return None; // need at least two clients to cycle
        }
        if self.layout_conf().follow_focus && self.clients.would_wrap(direction) {
            return None; // When following focus, don't allow wrapping focus
        }

        let prev = *self.clients.focused()?;
        let new = *self.clients.cycle_focus(direction)?;

        if prev != new {
            Some((prev, new))
        } else {
            None
        }
    }

    /**
     * Drag the focused client through the stack, retaining focus
     */
    pub fn drag_client(&mut self, direction: Direction) -> Option<WinId> {
        if self.layout_conf().follow_focus && self.clients.would_wrap(direction) {
            return None; // When following focus, don't allow wrapping focus
        }
        self.clients.drag_focused(direction).map(|c| *c)
    }

    pub fn update_max_main(&mut self, change: Change) {
        if let Some(layout) = self.layouts.focused_mut() {
            layout.update_max_main(change);
        }
    }

    pub fn update_main_ratio(&mut self, change: Change, step: f32) {
        if let Some(layout) = self.layouts.focused_mut() {
            layout.update_main_ratio(change, step);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::Direction;
    use crate::layout::*;

    fn test_layouts() -> Vec<Layout> {
        vec![Layout::new("t", LayoutConf::default(), mock_layout, 1, 0.6)]
    }

    fn add_n_clients(ws: &mut Workspace, n: usize) {
        for i in 0..n {
            let k = ((i + 1) * 10) as u32; // ensure win_id != index
            ws.add_client(k);
        }
    }

    #[test]
    fn ref_to_focused_client_when_empty() {
        let ws = Workspace::new("test", test_layouts());
        assert_eq!(ws.focused_client(), None);
    }

    #[test]
    fn ref_to_focused_client_when_populated() {
        let mut ws = Workspace::new("test", test_layouts());
        ws.clients = Ring::new(vec![42, 123]);

        let c = ws.focused_client().expect("should have had a client for 0");
        assert_eq!(c, 42);

        ws.clients.cycle_focus(Direction::Forward);
        let c = ws.focused_client().expect("should have had a client for 1");
        assert_eq!(c, 123);
    }

    #[test]
    fn removing_a_client_when_present() {
        let mut ws = Workspace::new("test", test_layouts());
        ws.clients = Ring::new(vec![13, 42]);

        let removed = ws
            .remove_client(42)
            .expect("should have had a client for id=42");
        assert_eq!(removed, 42);
    }

    #[test]
    fn removing_a_client_when_not_present() {
        let mut ws = Workspace::new("test", test_layouts());
        ws.clients = Ring::new(vec![13]);

        let removed = ws.remove_client(42);
        assert_eq!(removed, None, "got a client by the wrong ID");
    }

    #[test]
    fn adding_a_client() {
        let mut ws = Workspace::new("test", test_layouts());
        add_n_clients(&mut ws, 3);
        let ids: Vec<WinId> = ws.clients.iter().map(|c| *c).collect();
        assert_eq!(ids, vec![30, 20, 10], "not pushing at the top of the stack")
    }

    #[test]
    fn applying_a_layout_gives_one_action_per_client() {
        let mut ws = Workspace::new("test", test_layouts());
        ws.clients = Ring::new(vec![1, 2, 3]);
        let client_map = map! {
            1 => Client::new(1, "".into(), 1, false),
            2 => Client::new(2, "".into(), 1, false),
            3 => Client::new(3, "".into(), 1, false),
        };
        let actions = ws.arrange(&Region::new(0, 0, 2000, 1000), &client_map);
        assert_eq!(actions.len(), 3, "actions are not 1-1 for clients")
    }

    #[test]
    fn dragging_a_client_forward() {
        let mut ws = Workspace::new("test", test_layouts());
        ws.clients = Ring::new(vec![1, 2, 3, 4]);
        assert_eq!(ws.focused_client(), Some(1));

        assert_eq!(ws.drag_client(Direction::Forward), Some(1));
        assert_eq!(ws.clients.as_vec(), vec![2, 1, 3, 4]);

        assert_eq!(ws.drag_client(Direction::Forward), Some(1));
        assert_eq!(ws.clients.as_vec(), vec![2, 3, 1, 4]);

        assert_eq!(ws.drag_client(Direction::Forward), Some(1));
        assert_eq!(ws.clients.as_vec(), vec![2, 3, 4, 1]);

        assert_eq!(ws.drag_client(Direction::Forward), Some(1));
        assert_eq!(ws.clients.as_vec(), vec![1, 2, 3, 4]);

        assert_eq!(ws.focused_client(), Some(1));
    }

    #[test]
    fn dragging_non_index_0_client_backward() {
        let mut ws = Workspace::new("test", test_layouts());
        ws.clients = Ring::new(vec![1, 2, 3, 4]);
        ws.focus_client(3);
        assert_eq!(ws.focused_client(), Some(3));

        assert_eq!(ws.drag_client(Direction::Backward), Some(3));
        assert_eq!(ws.clients.as_vec(), vec![1, 3, 2, 4]);

        assert_eq!(ws.drag_client(Direction::Backward), Some(3));
        assert_eq!(ws.clients.as_vec(), vec![3, 1, 2, 4]);

        assert_eq!(ws.drag_client(Direction::Backward), Some(3));
        assert_eq!(ws.clients.as_vec(), vec![1, 2, 4, 3]);

        assert_eq!(ws.drag_client(Direction::Backward), Some(3));
        assert_eq!(ws.clients.as_vec(), vec![1, 2, 3, 4]);

        assert_eq!(ws.focused_client(), Some(3));
    }
}
