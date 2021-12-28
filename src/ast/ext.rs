use crate::static_graphql::query::{self};
use crate::static_graphql::schema::{
    self, Field, InterfaceType, ObjectType, TypeDefinition, UnionType,
};

use super::{get_named_type, TypeInfoElementRef};

pub trait AstNodeWithFields {
    fn find_field(&self, name: String) -> Option<&Field>;
}

impl AstNodeWithFields for ObjectType {
    fn find_field(&self, name: String) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

impl AstNodeWithFields for InterfaceType {
    fn find_field(&self, name: String) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

impl AstNodeWithFields for UnionType {
    fn find_field(&self, _name: String) -> Option<&Field> {
        None
    }
}

pub trait AstTypeRef {
    fn named_type(&self) -> String;
}

impl AstTypeRef for query::Type {
    fn named_type(&self) -> String {
        get_named_type(self)
    }
}

#[derive(Debug, Clone)]
pub enum CompositeType {
    Object(schema::ObjectType),
    Interface(schema::InterfaceType),
    Union(schema::UnionType),
}

impl TypeInfoElementRef<CompositeType> {
    pub fn find_field(&self, name: String) -> Option<&Field> {
        match self {
            TypeInfoElementRef::Empty => None,
            TypeInfoElementRef::Ref(composite_type) => composite_type.find_field(name),
        }
    }
}

impl CompositeType {
    pub fn find_field(&self, name: String) -> Option<&Field> {
        match self {
            CompositeType::Object(o) => o.find_field(name),
            CompositeType::Interface(i) => i.find_field(name),
            CompositeType::Union(u) => u.find_field(name),
        }
    }

    pub fn from_type_definition(t: &schema::TypeDefinition) -> Option<Self> {
        match t {
            schema::TypeDefinition::Object(o) => Some(CompositeType::Object(o.clone())),
            schema::TypeDefinition::Interface(i) => Some(CompositeType::Interface(i.clone())),
            schema::TypeDefinition::Union(u) => Some(CompositeType::Union(u.clone())),
            _ => None,
        }
    }
}

pub trait AbstractTypeDefinitionExtension {
    fn is_implemented_by(&self, other_type: &dyn ImplementingInterfaceExtension) -> bool;
}

pub trait TypeDefinitionExtension {
    fn is_leaf_type(&self) -> bool;
    fn is_composite_type(&self) -> bool;
    fn is_input_type(&self) -> bool;
    fn is_abstract_type(&self) -> bool;
    fn name(&self) -> String;
}

pub trait ImplementingInterfaceExtension {
    fn interfaces(&self) -> Vec<String>;
}

impl ImplementingInterfaceExtension for TypeDefinition {
    fn interfaces(&self) -> Vec<String> {
        match self {
            schema::TypeDefinition::Object(o) => o.interfaces(),
            schema::TypeDefinition::Interface(i) => i.interfaces(),
            _ => vec![],
        }
    }
}

impl ImplementingInterfaceExtension for InterfaceType {
    fn interfaces(&self) -> Vec<String> {
        self.implements_interfaces.clone()
    }
}

impl ImplementingInterfaceExtension for ObjectType {
    fn interfaces(&self) -> Vec<String> {
        self.implements_interfaces.clone()
    }
}

pub trait UnionTypeExtension {
    fn has_sub_type(&self, other_type_name: &String) -> bool;
}

impl UnionTypeExtension for UnionType {
    fn has_sub_type(&self, other_type_name: &String) -> bool {
        self.types.iter().find(|v| other_type_name.eq(*v)).is_some()
    }
}

impl AbstractTypeDefinitionExtension for InterfaceType {
    fn is_implemented_by(&self, other_type: &dyn ImplementingInterfaceExtension) -> bool {
        other_type
            .interfaces()
            .iter()
            .find(|v| self.name.eq(*v))
            .is_some()
    }
}

impl TypeDefinitionExtension for CompositeType {
    fn is_leaf_type(&self) -> bool {
        false
    }

    fn is_composite_type(&self) -> bool {
        true
    }

    fn is_input_type(&self) -> bool {
        false
    }

    fn name(&self) -> String {
        match self {
            CompositeType::Object(o) => o.name.clone(),
            CompositeType::Interface(i) => i.name.clone(),
            CompositeType::Union(u) => u.name.clone(),
        }
    }

    fn is_abstract_type(&self) -> bool {
        match self {
            CompositeType::Object(_o) => false,
            CompositeType::Interface(_i) => true,
            CompositeType::Union(_u) => true,
        }
    }
}

impl TypeDefinitionExtension for schema::TypeDefinition {
    fn name(&self) -> String {
        match self {
            schema::TypeDefinition::Object(o) => o.name.clone(),
            schema::TypeDefinition::Interface(i) => i.name.clone(),
            schema::TypeDefinition::Union(u) => u.name.clone(),
            schema::TypeDefinition::Scalar(s) => s.name.clone(),
            schema::TypeDefinition::Enum(e) => e.name.clone(),
            schema::TypeDefinition::InputObject(i) => i.name.clone(),
        }
    }

    fn is_abstract_type(&self) -> bool {
        match self {
            schema::TypeDefinition::Object(_o) => false,
            schema::TypeDefinition::Interface(_i) => true,
            schema::TypeDefinition::Union(_u) => true,
            schema::TypeDefinition::Scalar(_u) => false,
            schema::TypeDefinition::Enum(_u) => false,
            schema::TypeDefinition::InputObject(_u) => false,
        }
    }

    fn is_leaf_type(&self) -> bool {
        match self {
            schema::TypeDefinition::Object(_o) => false,
            schema::TypeDefinition::Interface(_i) => false,
            schema::TypeDefinition::Union(_u) => false,
            schema::TypeDefinition::Scalar(_u) => true,
            schema::TypeDefinition::Enum(_u) => true,
            schema::TypeDefinition::InputObject(_u) => false,
        }
    }

    fn is_input_type(&self) -> bool {
        match self {
            schema::TypeDefinition::Object(_o) => false,
            schema::TypeDefinition::Interface(_i) => false,
            schema::TypeDefinition::Union(_u) => false,
            schema::TypeDefinition::Scalar(_u) => true,
            schema::TypeDefinition::Enum(_u) => true,
            schema::TypeDefinition::InputObject(_u) => true,
        }
    }

    fn is_composite_type(&self) -> bool {
        match self {
            schema::TypeDefinition::Object(_o) => true,
            schema::TypeDefinition::Interface(_i) => true,
            schema::TypeDefinition::Union(_u) => true,
            schema::TypeDefinition::Scalar(_u) => false,
            schema::TypeDefinition::Enum(_u) => false,
            schema::TypeDefinition::InputObject(_u) => false,
        }
    }
}

pub trait AstNodeWithName {
    fn node_name(&self) -> Option<String>;
}

impl AstNodeWithName for query::OperationDefinition {
    fn node_name(&self) -> Option<String> {
        match self {
            query::OperationDefinition::Query(q) => q.name.clone(),
            query::OperationDefinition::SelectionSet(_s) => None,
            query::OperationDefinition::Mutation(m) => m.name.clone(),
            query::OperationDefinition::Subscription(s) => s.name.clone(),
        }
    }
}

impl AstNodeWithName for query::FragmentDefinition {
    fn node_name(&self) -> Option<String> {
        Some(self.name.clone())
    }
}
