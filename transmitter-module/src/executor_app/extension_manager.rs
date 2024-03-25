use libloading::{Library, Symbol};
use log::{error, info};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use transmitter_common::data::{ProtocolId, ProtocolIdImpl};

use transmitter_common::protocol_extension::{ProtocolExtension, GET_EXTENSION_EXPORT};

use super::error::ExecutorError;

pub(super) struct ExtensionManager {
    libs: Vec<Library>,
    extensions: BTreeMap<&'static ProtocolIdImpl, &'static dyn ProtocolExtension>,
}

impl ExtensionManager {
    pub(super) fn try_new(extensions_path: Vec<String>) -> Result<ExtensionManager, ExecutorError> {
        let mut extensions = ExtensionManager {
            libs: vec![],
            extensions: BTreeMap::default(),
        };
        unsafe { extensions.load_extensions(extensions_path)? };
        Ok(extensions)
    }

    unsafe fn load_extensions(
        &mut self,
        extension_paths: Vec<String>,
    ) -> Result<(), ExecutorError> {
        for ref extension_path in extension_paths {
            let lib = Library::new(extension_path).map_err(|err| {
                error!("Failed to load library from path: {}, error: {}", extension_path, err);
                ExecutorError::Extensions
            })?;
            let get_extension: Symbol<extern "C" fn() -> &'static dyn ProtocolExtension> =
                lib.get(GET_EXTENSION_EXPORT.as_bytes()).map_err(|err| {
                    error!(
                        "Failed to get `{}` export from: {}, error: {}",
                        GET_EXTENSION_EXPORT, extension_path, err
                    );
                    ExecutorError::Extensions
                })?;
            let extension: &'static dyn ProtocolExtension = get_extension();
            let protocol_id = extension.get_protocol_id();

            let Entry::Vacant(entry) = self.extensions.entry(protocol_id) else {
                error!("Extension with protocol_id exists: {}", ProtocolId(*protocol_id));
                return Err(ExecutorError::Extensions);
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

    pub(super) fn get_extension(
        &self,
        protocol_id: &ProtocolId,
    ) -> Option<&'static dyn ProtocolExtension> {
        self.extensions.get(&protocol_id.0).copied()
    }
}
