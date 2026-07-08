use crate::{
    attributes::cell::{PackKeyId, PackValueCell},
    category::{
        id::{FullIdRef, IdNameSeg},
        CategoryId,
    },
    script::{script_unimpl, Result},
};

#[cfg(todo)]
pub trait ScriptApiMenu {
    fn root_menu(&self) -> Result<Self::Menu> {
        script_unimpl!()
    }
    type Menu: MenuInstance;
}
#[allow(unused_variables)]
pub trait MenuDesc {
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> Result<Option<PackValueCell>> {
        script_unimpl!()
    }

    fn get_id(&self) -> Result<CategoryId> {
        script_unimpl!()
    }
}
#[allow(unused_variables)]
pub trait MenuHandle: MenuDesc {
    fn get_check_state(&self) -> Result<Option<bool>> {
        script_unimpl!()
    }
}
#[allow(unused_variables)]
pub trait MenuHandleMut: MenuHandle {
    fn set_menu_attr_dyn(&self, v: PackValueCell) -> Result<()> {
        script_unimpl!()
    }

    fn set_check_state(&self, v: Option<bool>) -> Result<()> {
        script_unimpl!()
    }
}
#[allow(unused_variables)]
pub trait MenuInstance: MenuDesc {
    fn register_id(&self, id: CategoryId) -> Result<Self::RegisteredMenu> {
        script_unimpl!()
    }
    type RegisteredMenu: MenuHandleMut;

    fn lookup_id(&self, id: &FullIdRef) -> Result<Option<Self::Menu>> {
        script_unimpl!()
    }
    type Menu: MenuHandle;

    fn remove_id(&self, id: &FullIdRef, recursive: bool) -> Result<()> {
        script_unimpl!()
    }

    /// produce an unused ID
    fn gen_id(&self, parent: Option<&FullIdRef>, name: Option<&IdNameSeg>) -> Result<String> {
        script_unimpl!()
    }

    #[cfg(todo)]
    fn iter_submenus_of();
}
