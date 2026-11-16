use gpui_component::Icon;
use gpui_kit::assets::IconName as Lucide;

/// Unified Lucide icon constructors backed by `gpui_kit_assets::AllAssets`.
pub mod icons {
    use super::*;

    pub fn sidebar_close() -> Icon {
        Icon::new(Lucide::PanelLeftClose)
    }

    pub fn sidebar_open() -> Icon {
        Icon::new(Lucide::PanelLeftOpen)
    }

    pub fn cpu() -> Icon {
        Icon::new(Lucide::Cpu)
    }

    pub fn user() -> Icon {
        Icon::new(Lucide::UserRound)
    }

    pub fn broom() -> Icon {
        Icon::new(Lucide::BroomSparkles)
    }

    pub fn play() -> Icon {
        Icon::new(Lucide::Play)
    }

    pub fn square_stop() -> Icon {
        Icon::new(Lucide::SquareStop)
    }

    pub fn alert() -> Icon {
        Icon::new(Lucide::CircleAlert)
    }

    pub fn plus() -> Icon {
        Icon::new(Lucide::Plus)
    }

    pub fn close() -> Icon {
        Icon::new(Lucide::X)
    }

    pub fn settings() -> Icon {
        Icon::new(Lucide::SlidersHorizontal)
    }

    pub fn folder() -> Icon {
        Icon::new(Lucide::Folder)
    }

    pub fn folder_open() -> Icon {
        Icon::new(Lucide::FolderOpen)
    }

    pub fn refresh() -> Icon {
        Icon::new(Lucide::RefreshCw)
    }

    pub fn trash() -> Icon {
        Icon::new(Lucide::Trash)
    }

    pub fn copy() -> Icon {
        Icon::new(Lucide::Copy)
    }

    pub fn paperclip() -> Icon {
        Icon::new(Lucide::Paperclip)
    }

    pub fn send() -> Icon {
        Icon::new(Lucide::SendHorizontal)
    }

    pub fn chevron_down() -> Icon {
        Icon::new(Lucide::ChevronDown)
    }

    pub fn chevron_right() -> Icon {
        Icon::new(Lucide::ChevronRight)
    }

    pub fn brain() -> Icon {
        Icon::new(Lucide::Brain)
    }

    pub fn terminal() -> Icon {
        Icon::new(Lucide::SquareTerminal)
    }

    pub fn file_text() -> Icon {
        Icon::new(Lucide::FileText)
    }

    pub fn file_pen() -> Icon {
        Icon::new(Lucide::FilePenLine)
    }

    pub fn search() -> Icon {
        Icon::new(Lucide::Search)
    }

    pub fn globe() -> Icon {
        Icon::new(Lucide::Globe)
    }

    pub fn sparkles() -> Icon {
        Icon::new(Lucide::Sparkles)
    }

    pub fn memory() -> Icon {
        Icon::new(Lucide::MemoryStick)
    }

    pub fn check() -> Icon {
        Icon::new(Lucide::Check)
    }

    pub fn image() -> Icon {
        Icon::new(Lucide::FileImage)
    }

    pub fn list_checks() -> Icon {
        Icon::new(Lucide::MessageCircleQuestionMark)
    }
}
