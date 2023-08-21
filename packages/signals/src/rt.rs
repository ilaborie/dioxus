use std::rc::Rc;

use dioxus_core::prelude::{
    consume_context, consume_context_from_scope, current_scope_id, has_context, provide_context,
    provide_context_to_scope, provide_root_context,
};
use dioxus_core::ScopeId;

use generational_box::{
    BorrowError, BorrowMutError, GenerationalBox, GenerationalRef, GenerationalRefMut, Owner, Store,
};

fn current_store() -> Store {
    match consume_context() {
        Some(rt) => rt,
        None => {
            let store = Store::default();
            provide_root_context(store).expect("in a virtual dom")
        }
    }
}

fn current_owner() -> Rc<Owner> {
    match has_context() {
        Some(rt) => rt,
        None => {
            let owner = Rc::new(current_store().owner());
            provide_context(owner).expect("in a virtual dom")
        }
    }
}

fn owner_in_scope(scope: ScopeId) -> Rc<Owner> {
    match consume_context_from_scope(scope) {
        Some(rt) => rt,
        None => {
            let owner = Rc::new(current_store().owner());
            provide_context_to_scope(scope, owner).expect("in a virtual dom")
        }
    }
}

/// CopyValue is a wrapper around a value to make the value mutable and Copy.
///
/// It is internally backed by [`generational_box::GenerationalBox`].
pub struct CopyValue<T: 'static> {
    pub(crate) value: GenerationalBox<T>,
    origin_scope: ScopeId,
}

#[cfg(feature = "serde")]
impl<T: 'static> serde::Serialize for CopyValue<T>
where
    T: serde::Serialize,
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.read().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: 'static> serde::Deserialize<'de> for CopyValue<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = T::deserialize(deserializer)?;

        Ok(Self::new(value))
    }
}

impl<T: 'static> CopyValue<T> {
    /// Create a new CopyValue. The value will be stored in the current component.
    ///
    /// Once the component this value is created in is dropped, the value will be dropped.
    #[track_caller]
    pub fn new(value: T) -> Self {
        let owner = current_owner();

        Self {
            value: owner.insert(value),
            origin_scope: current_scope_id().expect("in a virtual dom"),
        }
    }

    pub(crate) fn new_with_caller(
        value: T,
        #[cfg(debug_assertions)] caller: &'static std::panic::Location<'static>,
    ) -> Self {
        let owner = current_owner();

        Self {
            value: owner.insert_with_caller(
                value,
                #[cfg(debug_assertions)]
                caller,
            ),
            origin_scope: current_scope_id().expect("in a virtual dom"),
        }
    }

    /// Create a new CopyValue. The value will be stored in the given scope. When the specified scope is dropped, the value will be dropped.
    pub fn new_in_scope(value: T, scope: ScopeId) -> Self {
        let owner = owner_in_scope(scope);

        Self {
            value: owner.insert(value),
            origin_scope: scope,
        }
    }

    pub(crate) fn invalid() -> Self {
        let owner = current_owner();

        Self {
            value: owner.invalid(),
            origin_scope: current_scope_id().expect("in a virtual dom"),
        }
    }

    /// Get the scope this value was created in.
    pub fn origin_scope(&self) -> ScopeId {
        self.origin_scope
    }

    /// Try to read the value. If the value has been dropped, this will return None.
    #[track_caller]
    pub fn try_read(&self) -> Result<GenerationalRef<'_, T>, BorrowError> {
        self.value.try_read()
    }

    /// Read the value. If the value has been dropped, this will panic.
    #[track_caller]
    pub fn read(&self) -> GenerationalRef<'_, T> {
        self.value.read()
    }

    /// Try to write the value. If the value has been dropped, this will return None.
    #[track_caller]
    pub fn try_write(&self) -> Result<GenerationalRefMut<'_, T>, BorrowMutError> {
        self.value.try_write()
    }

    /// Write the value. If the value has been dropped, this will panic.
    #[track_caller]
    pub fn write(&self) -> GenerationalRefMut<'_, T> {
        self.value.write()
    }

    /// Set the value. If the value has been dropped, this will panic.
    pub fn set(&mut self, value: T) {
        *self.write() = value;
    }

    /// Run a function with a reference to the value. If the value has been dropped, this will panic.
    pub fn with<O>(&self, f: impl FnOnce(&T) -> O) -> O {
        let write = self.read();
        f(&*write)
    }

    /// Run a function with a mutable reference to the value. If the value has been dropped, this will panic.
    pub fn with_mut<O>(&self, f: impl FnOnce(&mut T) -> O) -> O {
        let mut write = self.write();
        f(&mut *write)
    }
}

impl<T: Clone + 'static> CopyValue<T> {
    /// Get the value. If the value has been dropped, this will panic.
    pub fn value(&self) -> T {
        self.read().clone()
    }
}

impl<T: 'static> PartialEq for CopyValue<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.ptr_eq(&other.value)
    }
}
