use libloading::{Library, Symbol};
use log::{error, info};
use std::{
    cell::RefCell,
    collections::{btree_map::Entry, BTreeMap},
    mem::swap,
    ops::DerefMut,
};

use transmitter_common::{
    data::{ProtocolId, ProtocolIdImpl},
    protocol_extension::{ProtocolExtension, GET_EXTENSION_EXPORT},
};

use super::error::ExecutorError;

pub(super) struct ExtensionManager {
    extension_manager_impl: RefCell<ExtensionManagerImpl>,
}

#[derive(Default)]
struct ExtensionManagerImpl {
    libs: Vec<Library>,
    extensions: BTreeMap<&'static ProtocolIdImpl, &'static dyn ProtocolExtension>,
}

impl ExtensionManagerImpl {
    unsafe fn load_extensions(
        &mut self,
        extension_paths: Vec<String>,
    ) -> Result<(), ExecutorError> {
        for ref extension_path in extension_paths {
            let lib = Library::new(extension_path).map_err(|err| {
                error!("Failed to load library from path: {}, error: {}", extension_path, err);
                ExecutorError::ExtensionMng
            })?;
            let get_extension: Symbol<extern "C" fn() -> &'static dyn ProtocolExtension> =
                lib.get(GET_EXTENSION_EXPORT.as_bytes()).map_err(|err| {
                    error!(
                        "Failed to get `{}` export from: {}, error: {}",
                        GET_EXTENSION_EXPORT, extension_path, err
                    );
                    ExecutorError::ExtensionMng
                })?;
            let extension: &'static dyn ProtocolExtension = get_extension();
            let protocol_id = extension.get_protocol_id();

            let Entry::Vacant(entry) = self.extensions.entry(protocol_id) else {
                error!("Extension with protocol_id exists: {}", ProtocolId(*protocol_id));
                return Err(ExecutorError::ExtensionMng);
            };

            entry.insert(extension);

            info!(
                "Extension: {} - has been registered for protocol_id: {}",
                extension_path,
                ProtocolId(*protocol_id)
            );
            self.libs.push(lib);
        }
        Ok(())
    }
}

impl ExtensionManager {
    pub(super) fn new(extension_paths: Vec<String>) -> ExtensionManager {
        let extension_mng = ExtensionManager {
            extension_manager_impl: RefCell::new(ExtensionManagerImpl::default()),
        };
        extension_mng.on_update_extensions(extension_paths);
        extension_mng
    }

    pub(super) fn on_update_extensions(&self, extension_paths: Vec<String>) {
        let mut extensions = ExtensionManagerImpl::default();
        if let Err(err) = unsafe { extensions.load_extensions(extension_paths) } {
            error!("Failed to load extensions: {} - changes will not be applied", err);
        } else {
            swap(self.extension_manager_impl.borrow_mut().deref_mut(), &mut extensions);
        }
    }

    pub(super) fn get_extension(
        &self,
        protocol_id: &ProtocolId,
    ) -> Option<&'static dyn ProtocolExtension> {
        self.extension_manager_impl.borrow().extensions.get(&protocol_id.0).copied()
    }
}
