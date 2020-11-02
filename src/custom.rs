//! Custom directory checking

use failure::Error;

use std::{env, ffi, path::Path};

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CustomEntry {
    pub name: String,
    pub path: String,
    pub check_cmd: String,
    pub refresh_cmd: String,
}

impl CustomEntry {
    
    pub fn check(&self) -> Result<bool, Error> {
	let expanded_path: &str = &shellexpand::full(&self.path)?;

	// Save current dir
	let cwd = env::current_dir()?;

	// Move to custom dir
	env::set_current_dir(Path::new(&expanded_path))?;

	// Run the check command
	let command_result: libc::c_int;
	unsafe {
            command_result =
		libc::WEXITSTATUS(libc::system(ffi::CString::new(self.check_cmd.clone())?.as_ptr()));
	}

	let mut res = true;
	if command_result != libc::EXIT_SUCCESS {
            res = false;
	}

	// Return to original dir
	env::set_current_dir(Path::new(&cwd))?;

	Ok(res)
    }

    pub fn refresh(&self) -> Result<(), Error> {
	let expanded_path: &str = &shellexpand::full(&self.path)?;

	// Save current dir
	let cwd = env::current_dir()?;

	// Move to custom dir
	env::set_current_dir(Path::new(&expanded_path))?;

	// Run the check command
	let command_result: libc::c_int;
	unsafe {
            command_result =
		libc::WEXITSTATUS(libc::system(ffi::CString::new(self.refresh_cmd.clone())?.as_ptr()));
	}

	if command_result != libc::EXIT_SUCCESS {
            warn!(
		"{}: custom dir refresh command exited with code {}",
		self.path, command_result
            );
	}

	// Return to original dir
	env::set_current_dir(Path::new(&cwd))?;

	Ok(())
    }

}
