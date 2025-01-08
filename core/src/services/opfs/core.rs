use std::fmt::Debug;

use crate::Result;

use web_sys::{
    window, File, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
    FileSystemWritableFileStream,
};

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

pub struct OpfsCore {}

impl Debug for OpfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        panic!()
    }
}

impl OpfsCore {
    pub async fn store_file(file_name: &str, content: &[u8]) -> Result<(), JsValue> {
        // Access the OPFS
        let navigator = window().unwrap().navigator();
        let storage_manager = navigator.storage();
        let root: FileSystemDirectoryHandle = JsFuture::from(storage_manager.get_directory())
            .await?
            .dyn_into()?;

        let opt = FileSystemGetFileOptions::new();
        opt.set_create(true);

        // Create or get the file in the OPFS
        let file_handle: FileSystemFileHandle =
            JsFuture::from(root.get_file_handle_with_options(file_name, &opt))
                .await?
                .dyn_into()?;

        // Create a writable stream
        let writable: FileSystemWritableFileStream = JsFuture::from(file_handle.create_writable())
            .await?
            .dyn_into()?;

        // Write the content to the file
        JsFuture::from(
            writable
                .write_with_u8_array(content)
                .expect("failed to write file"),
        )
        .await?;

        // Close the writable stream
        JsFuture::from(writable.close()).await?;

        Ok(())
    }

    pub async fn read_file(file_name: &str) -> Result<Vec<u8>, JsValue> {
        // Access the OPFS
        let navigator = window().unwrap().navigator();
        let storage_manager = navigator.storage();
        let root: FileSystemDirectoryHandle = JsFuture::from(storage_manager.get_directory())
            .await?
            .dyn_into()?;

        // Get the file handle
        let file_handle: FileSystemFileHandle = JsFuture::from(root.get_file_handle(file_name))
            .await?
            .dyn_into()?;

        // Get the file from the handle
        let file: File = JsFuture::from(file_handle.get_file()).await?.dyn_into()?;
        let array_buffer = JsFuture::from(file.array_buffer()).await?;

        // Convert the ArrayBuffer to a Vec<u8>
        let u8_array = js_sys::Uint8Array::new(&array_buffer);
        let mut vec = vec![0; u8_array.length() as usize];
        u8_array.copy_to(&mut vec[..]);

        Ok(vec)
    }
}
