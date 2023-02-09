use std::{
    borrow::Cow,
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use anyhow::Context;
use derive_more::From;

use crate::utils::{error::ResultExt, mutex::Mutex};

use super::result::{TestError, TestResult};

#[allow(clippy::type_complexity)]
pub struct GenericTestNode<C> {
    parent: Option<Weak<ParentTestNode>>,
    name: Cow<'static, str>,
    full_name: String,
    content: C,
    result: Mutex<Option<TestResult>>,
    pub on_complete: Option<Box<dyn Fn(&GenericTestNode<C>, &TestResult) + Send + Sync>>,
}

pub type ParentTestNode = GenericTestNode<Mutex<ParentNodeContent>>;
pub type LeafTestNode = GenericTestNode<()>;

#[derive(From)]
pub enum TestNode {
    Parent(Arc<ParentTestNode>),
    Leaf(Arc<LeafTestNode>),
}

#[derive(Default)]
pub struct ParentNodeContent {
    children: BTreeMap<Cow<'static, str>, TestNode>,
}

impl ParentTestNode {
    pub fn new_root<F>(name: impl Into<Cow<'static, str>>, on_complete: F) -> Arc<Self>
    where
        F: Fn(&ParentTestNode, &TestResult) + Send + Sync + 'static,
    {
        let name = name.into();
        Arc::new(Self {
            name: name.clone(),
            full_name: String::from(name),
            content: Mutex::new(ParentNodeContent::default()),
            on_complete: Some(Box::new(on_complete)),
            parent: None,
            result: Mutex::new(None),
        })
    }

    fn new_child<C>(&self, child: GenericTestNode<C>) -> Arc<GenericTestNode<C>>
    where
        TestNode: From<Arc<GenericTestNode<C>>>,
    {
        let child = Arc::new(child);
        let mut content = self.content.lock();
        let ret_child = child.clone();
        let old_value = content
            .children
            .insert(child.name.clone(), TestNode::from(child));
        debug_assert!(old_value.is_none());
        ret_child
    }

    pub fn new_child_parent(
        self: &Arc<Self>,
        name: impl Into<Cow<'static, str>>,
    ) -> Arc<ParentTestNode> {
        let name = name.into();
        self.new_child(Self {
            parent: Some(Arc::downgrade(self)),
            full_name: format!("{}.{}", self.name, name),
            name,
            result: Mutex::new(None),
            content: Mutex::new(ParentNodeContent::default()),
            on_complete: None,
        })
    }

    pub fn new_child_leaf(
        self: &Arc<Self>,
        name: impl Into<Cow<'static, str>>,
    ) -> Arc<LeafTestNode> {
        let name = name.into();
        self.new_child(GenericTestNode {
            parent: Some(Arc::downgrade(self)),
            full_name: format!("{}.{}", self.name, name),
            name,
            result: Mutex::new(None),
            content: (),
            on_complete: None,
        })
    }

    fn update_child(&self, name: &str, new_result: TestResult) {
        {
            let lock = self.content.lock();
            let child = lock
                .children
                .get(name)
                .unwrap_or_else(|| panic!("child test node named {name} not found"));
            match child {
                TestNode::Parent(par) => *par.result.lock() = Some(new_result),
                TestNode::Leaf(leaf) => *leaf.result.lock() = Some(new_result),
            }
        }

        if let Some(result) = self.get_result() {
            if let Some(parent) = self
                .parent
                .as_ref()
                .and_then(|par| par.upgrade().context("parent node was dropped").log_warn())
            {
                parent.update_child(&self.name, result);
            }
        }
    }

    fn get_result(&self) -> Option<TestResult> {
        let lock = self.content.lock();
        let mut failed_tests = Vec::new();
        let mut pending_tests = Vec::new();
        for (name, node) in lock.children.iter() {
            let guard = match node {
                TestNode::Parent(par) => par.result.lock(),
                TestNode::Leaf(leaf) => leaf.result.lock(),
            };

            match *guard {
                Some(TestResult::Err(_)) => failed_tests.push(name.clone()),
                None => pending_tests.push(name.clone()),
                _ => {}
            }
        }

        if pending_tests.is_empty() {
            if failed_tests.is_empty() {
                Some(TestResult::Ok(()))
            } else {
                Some(TestResult::Err(TestError::ChildFailedError(failed_tests)))
            }
        } else {
            None
        }
    }
}

impl LeafTestNode {
    pub fn update(&self, result: TestResult) {
        tracing::info!(
            "test `{}` finished with result {:?}",
            self.full_name,
            result
        );
        if let Ok(parent) = self
            .parent
            .as_ref()
            .expect("leaf node without parent")
            .upgrade()
            .context("parent node was dropped")
        {
            parent.update_child(&self.name, result);
        }
    }
}
