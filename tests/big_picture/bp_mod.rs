use dlss_studio::big_picture::launch_args_request_big_picture;

#[test]
fn test_launch_args_request_big_picture() {
    let args_bp = vec!["app.exe".to_string(), "--big-picture".to_string()];
    assert!(launch_args_request_big_picture(&args_bp));

    let args_tv = vec!["app.exe".to_string(), "--tv".to_string()];
    assert!(launch_args_request_big_picture(&args_tv));

    let args_short = vec!["app.exe".to_string(), "-bp".to_string()];
    assert!(launch_args_request_big_picture(&args_short));

    let args_normal = vec!["app.exe".to_string(), "--restore".to_string()];
    assert!(!launch_args_request_big_picture(&args_normal));
}
