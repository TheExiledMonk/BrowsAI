use browsai_uploads::{normalize_filename, UploadError};

#[test]
fn upload_filenames_reject_paths_and_control_characters() {
    assert_eq!(normalize_filename("  photo.png ").unwrap(), "photo.png");
    assert_eq!(
        normalize_filename("folder/photo.png"),
        Err(UploadError::InvalidFilename)
    );
    assert_eq!(
        normalize_filename("photo\0.png"),
        Err(UploadError::InvalidFilename)
    );
}
