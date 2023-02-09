struct ExtensionRegistrar {
    extensions: HashMap<String, ExtensionProxy>,
    lib: Rc<Library>,
}

impl ExtesionRegistrar {
    fn new(lib: Rc<Library>) -> ExtesionRegistrar {
        ExtesionRegistrar {
            lib,
            extensions: HashMap::default(),
        }
    }
}

impl extensions::ExtensionRegistrar for ExtesionRegistrar {
    fn register(&mut self, name: &str, extension: Box<dyn Extension>) {
        let proxy = ExtensionProxy {
            extension,
            _lib: Rc::clone(&self.lib),
        };
        self.extensions.insert(name.to_string(), proxy);
    }
}

#[derive(Default)]
pub struct AvailableExtensions {
    extensions: HashMap<String, ExtensionProxy>,
    libraries: Vec<Rc<Library>>,
}

impl AvailableExtensions {
    pub fn new() -> AvailableExtensions {
        AvailableExtensions::default()
    }

    /// # Safety
    ///
    /// An extension **must** be implemented using the
    /// [`extensions::export_extension!()`] macro. Trying manually implement
    /// a plugin without going through that macro will result in undefined
    /// behaviour.
    pub unsafe fn load<P: AsRef<OsStr>>(&mut self, library_path: P) -> io::Result<()> {
        // load the library into memory
        let library = Rc::new(Library::new(library_path)?);

        // get a pointer to the plugin_declaration symbol.
        let decl = library.get::<*mut Extension>(b"extension_entry\0")?.read();

        // version checks to prevent accidental ABI incompatibilities
        if decl.rustc_version != extensions::RUSTC_VERSION
            || decl.core_version != extensions::CORE_VERSION
        {
            return Err(io::Error::new(io::ErrorKind::Other, "Version mismatch"));
        }

        let mut registrar = ExtensionRegistrar::new(Rc::clone(&library));

        (decl.register)(&mut registrar);

        // add all loaded plugins to the functions map
        self.extensions.extend(registrar.extensions);
        // and make sure ExternalFunctions keeps a reference to the library
        self.libraries.push(library);

        Ok(())
    }
}
