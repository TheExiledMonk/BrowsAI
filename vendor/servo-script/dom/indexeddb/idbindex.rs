/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use dom_struct::dom_struct;
use js::context::JSContext;
use js::conversions::ToJSValConvertible;
use js::gc::MutableHandleValue;
use js::rust::HandleValue;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::IDBIndexBinding::IDBIndexMethods;
use script_bindings::codegen::GenericBindings::IDBObjectStoreBinding::IDBObjectStoreMethods;
use script_bindings::codegen::GenericBindings::IDBTransactionBinding::IDBTransactionMode;
use script_bindings::error::{Error, ErrorResult};
use script_bindings::reflector::{Reflector, reflect_dom_object_with_cx};
use script_bindings::str::DOMString;

use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::codegen::Bindings::IDBCursorBinding::IDBCursorDirection;
use crate::dom::globalscope::GlobalScope;
use crate::dom::idbobjectstore::KeyPath;
use crate::dom::indexeddb::idbobjectstore::IDBObjectStore;
use crate::dom::indexeddb::idbrequest::IDBRequest;
use crate::dom::indexeddb::idbcursor::ObjectStoreOrIndex as CursorSource;

#[dom_struct]
pub(crate) struct IDBIndex {
    reflector_: Reflector,
    object_store: Dom<IDBObjectStore>,
    name: DomRefCell<DOMString>,
    multi_entry: bool,
    unique: bool,
    key_path: KeyPath,
}

impl IDBIndex {
    pub fn new_inherited(
        object_store: &IDBObjectStore,
        name: DOMString,
        multi_entry: bool,
        unique: bool,
        key_path: KeyPath,
    ) -> IDBIndex {
        IDBIndex {
            reflector_: Reflector::new(),
            object_store: Dom::from_ref(object_store),
            name: DomRefCell::new(name),
            multi_entry,
            unique,
            key_path,
        }
    }

    pub fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        object_store: &IDBObjectStore,
        name: DOMString,
        multi_entry: bool,
        unique: bool,
        key_path: KeyPath,
    ) -> DomRoot<IDBIndex> {
        reflect_dom_object_with_cx(
            Box::new(IDBIndex::new_inherited(
                object_store,
                name,
                multi_entry,
                unique,
                key_path,
            )),
            global,
            cx,
        )
    }

    pub(crate) fn cursor_key_path(&self) -> KeyPath {
        self.key_path.clone()
    }

    pub(crate) fn cursor_multi_entry(&self) -> bool {
        self.multi_entry
    }

    pub(crate) fn cursor_object_store(&self) -> DomRoot<IDBObjectStore> {
        self.object_store.as_rooted()
    }
}

impl IDBIndexMethods<crate::DomTypeHolder> for IDBIndex {
    /// <https://www.w3.org/TR/IndexedDB/#dom-idbindex-name>
    fn Name(&self) -> DOMString {
        self.name.borrow().clone()
    }

    /// <https://www.w3.org/TR/IndexedDB/#ref-for-dom-idbindex-name%E2%91%A2>
    fn SetName(&self, name: DOMString) -> ErrorResult {
        // Step 1: Let name be the given value.
        // Step 2: Let transaction be this’s transaction.
        let transaction = self.object_store.transaction();

        // Step 3: Let index be this’s index.
        // We do not have an explicit object representing the underlying index.

        // Step 4: If transaction is not an upgrade transaction, throw an "InvalidStateError" DOMException.
        if transaction.get_mode() != IDBTransactionMode::Versionchange {
            return Err(Error::InvalidState(Some(
                "Transaction is not an upgrade transaction".to_owned(),
            )));
        }

        // Step 5: If transaction’s state is not active, then throw a "TransactionInactiveError" DOMException.
        if !transaction.is_active() {
            return Err(Error::TransactionInactive(Some(
                "Transaction is not active while updating index name".to_owned(),
            )));
        }

        // Step 6: If index or index’s object store has been deleted, throw an "InvalidStateError" DOMException.
        let mut stored_name = self.name.borrow_mut();
        if !self.object_store.has_index(&stored_name) ||
            !transaction
                .get_db()
                .object_store_exists(&self.object_store.get_name())
        {
            return Err(Error::InvalidState(Some(
                "Index or its object store has been deleted".to_owned(),
            )));
        }

        // Step 7: If index’s name is equal to name, terminate these steps.
        if *stored_name == name {
            return Ok(());
        }

        // Step 8: If an index named name already exists in index’s object store, throw a "ConstraintError" DOMException.
        if self.object_store.has_index(&name) {
            return Err(Error::Constraint(Some(
                "An index with the given name already exists".to_owned(),
            )));
        }

        // Step 9: Set index’s name to name.
        self.object_store.rename_index(&stored_name, &name);

        // Step 10: Set this’s name to name.
        *stored_name = name;
        Ok(())
    }

    /// <https://www.w3.org/TR/IndexedDB/#dom-idbindex-objectstore>
    fn ObjectStore(&self) -> DomRoot<IDBObjectStore> {
        self.object_store.as_rooted()
    }

    /// <https://www.w3.org/TR/IndexedDB/#dom-idbindex-multientry>
    fn MultiEntry(&self) -> bool {
        self.multi_entry
    }

    /// <https://www.w3.org/TR/IndexedDB/#dom-idbindex-unique>
    fn Unique(&self) -> bool {
        self.unique
    }

    #[allow(unsafe_code)]
    fn OpenCursor(
        &self,
        query: HandleValue,
        direction: IDBCursorDirection,
    ) -> Result<DomRoot<IDBRequest>, Error> {
        let mut cx = unsafe { script_bindings::script_runtime::temp_cx() };
        self.object_store.open_cursor_with_source(
            &mut cx,
            query,
            direction,
            false,
            CursorSource::Index(Dom::from_ref(self)),
        )
    }

    #[allow(unsafe_code)]
    fn OpenKeyCursor(
        &self,
        query: HandleValue,
        direction: IDBCursorDirection,
    ) -> Result<DomRoot<IDBRequest>, Error> {
        let mut cx = unsafe { script_bindings::script_runtime::temp_cx() };
        self.object_store.open_cursor_with_source(
            &mut cx,
            query,
            direction,
            true,
            CursorSource::Index(Dom::from_ref(self)),
        )
    }

    /// Temporary compatibility path until the storage backend exposes native
    /// index-bulk operations.  The object-store request path preserves the
    /// asynchronous IDBRequest contract and prevents vendor feature-detection
    /// failures; index-specific filtering remains a follow-up backend task.
    #[allow(unsafe_code)]
    fn GetAll(&self, query: HandleValue, count: Option<u32>) -> Result<DomRoot<IDBRequest>, Error> {
        let mut cx = unsafe { script_bindings::script_runtime::temp_cx() };
        self.object_store.GetAll(&mut cx, query, count)
    }

    #[allow(unsafe_code)]
    fn GetAllKeys(&self, query: HandleValue, count: Option<u32>) -> Result<DomRoot<IDBRequest>, Error> {
        let mut cx = unsafe { script_bindings::script_runtime::temp_cx() };
        self.object_store.GetAllKeys(&mut cx, query, count)
    }

    /// <https://www.w3.org/TR/IndexedDB/#dom-idbindex-keypath>
    fn KeyPath(&self, cx: &mut JSContext, retval: MutableHandleValue) {
        match &self.key_path {
            KeyPath::String(string) => {
                string.safe_to_jsval(cx, retval);
            },
            KeyPath::StringSequence(sequence) => {
                sequence.safe_to_jsval(cx, retval);
            },
        }
    }
}
