//! The renderer membership machine, independent of a native object kind.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility { Hidden, Visible }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership { Detached, Hidden, Shown }

#[derive(Clone, Copy, Debug)]
pub enum PresentationEvent { Insert(Visibility), Remove, Present(Visibility) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendererAction { Keep, Add, Remove }

impl Membership {
    pub fn transition(self, event: PresentationEvent) -> (Self, RendererAction) {
        let next = match (self, event) {
            (_, PresentationEvent::Remove) => Self::Detached,
            (Self::Detached, PresentationEvent::Present(_)) => Self::Detached,
            (_, PresentationEvent::Insert(Visibility::Visible) | PresentationEvent::Present(Visibility::Visible)) => Self::Shown,
            (_, PresentationEvent::Insert(Visibility::Hidden) | PresentationEvent::Present(Visibility::Hidden)) => Self::Hidden,
        };
        let action = match (self, next) {
            (Self::Shown, Self::Shown) => RendererAction::Keep,
            (Self::Shown, _) => RendererAction::Remove,
            (_, Self::Shown) => RendererAction::Add,
            _ => RendererAction::Keep,
        };
        (next, action)
    }
}
