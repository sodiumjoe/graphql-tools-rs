use std::collections::{BTreeMap, HashMap};

use graphql_parser::query::TypeCondition;

use crate::static_graphql::{
    query::{self, *},
    schema::{self},
};

use super::{
    FieldByNameExtension, OperationDefinitionExtension, SchemaDocumentExtension, TypeExtension,
};
/// OperationVisitor
pub struct OperationVisitorContext<'a> {
    pub schema: &'a schema::Document,
    pub operation: &'a query::Document,
    pub known_fragments: HashMap<String, FragmentDefinition>,
    pub directives: HashMap<String, schema::DirectiveDefinition>,

    type_stack: Vec<Option<schema::TypeDefinition>>,
    parent_type_stack: Vec<Option<schema::TypeDefinition>>,
    input_type_stack: Vec<Option<schema::TypeDefinition>>,
    type_literal_stack: Vec<Option<Type>>,
    input_type_literal_stack: Vec<Option<Type>>,
}

impl<'a> OperationVisitorContext<'a> {
    pub fn new(operation: &'a Document, schema: &'a schema::Document) -> Self {
        OperationVisitorContext {
            schema,
            operation,
            type_stack: vec![],
            parent_type_stack: vec![],
            input_type_stack: vec![],
            type_literal_stack: vec![],
            input_type_literal_stack: vec![],
            known_fragments: HashMap::<String, FragmentDefinition>::from_iter(
                operation.definitions.iter().filter_map(|def| match def {
                    Definition::Fragment(fragment) => {
                        Some((fragment.name.clone(), fragment.clone()))
                    }
                    _ => None,
                }),
            ),
            directives: HashMap::<String, schema::DirectiveDefinition>::from_iter(
                schema.definitions.iter().filter_map(|def| match def {
                    schema::Definition::DirectiveDefinition(directive_def) => {
                        Some((directive_def.name.clone(), directive_def.clone()))
                    }
                    _ => None,
                }),
            ),
        }
    }

    pub fn with_type<Func>(&mut self, t: Option<Type>, func: Func)
    where
        Func: FnOnce(&mut OperationVisitorContext<'a>) -> (),
    {
        if let Some(ref t) = t {
            self.type_stack
                .push(self.schema.type_by_name(&t.inner_type()));
        } else {
            self.type_stack.push(None);
        }

        self.type_literal_stack.push(t);
        func(self);
        self.type_literal_stack.pop();
        self.type_stack.pop();
    }

    pub fn with_parent_type<Func>(&mut self, func: Func)
    where
        Func: FnOnce(&mut OperationVisitorContext<'a>) -> (),
    {
        self.parent_type_stack
            .push(self.type_stack.last().unwrap_or(&None).clone());
        func(self);
        self.parent_type_stack.pop();
    }

    pub fn with_input_type<Func>(&mut self, t: Option<Type>, func: Func)
    where
        Func: FnOnce(&mut OperationVisitorContext<'a>) -> (),
    {
        if let Some(ref t) = t {
            self.input_type_stack
                .push(self.schema.type_by_name(&t.inner_type()));
        } else {
            self.input_type_stack.push(None);
        }

        self.input_type_literal_stack.push(t);
        func(self);
        self.input_type_literal_stack.pop();
        self.input_type_stack.pop();
    }

    pub fn current_type(&self) -> Option<&schema::TypeDefinition> {
        self.type_stack.last().unwrap_or(&None).as_ref()
    }

    pub fn current_parent_type(&self) -> Option<&schema::TypeDefinition> {
        self.parent_type_stack.last().unwrap_or(&None).as_ref()
    }

    pub fn current_type_literal(&self) -> Option<&Type> {
        self.type_literal_stack.last().unwrap_or(&None).as_ref()
    }

    pub fn current_input_type_literal(&self) -> Option<&Type> {
        self.input_type_literal_stack
            .last()
            .unwrap_or(&None)
            .as_ref()
    }
}

pub fn visit_document<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    document: &Document,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    visitor.enter_document(context, user_context, document);
    visit_definitions(visitor, &document.definitions, context, user_context);
    visitor.leave_document(context, user_context, document);
}

fn visit_definitions<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    definitions: &Vec<Definition>,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    for definition in definitions {
        let schema_type_name = match definition {
            Definition::Fragment(fragment) => {
                let TypeCondition::On(name) = &fragment.type_condition;
                Some(name.clone())
            }
            Definition::Operation(operation) => match operation {
                OperationDefinition::Query(_) => Some(context.schema.query_type().name.clone()),
                OperationDefinition::SelectionSet(_) => {
                    Some(context.schema.query_type().name.clone())
                }
                OperationDefinition::Mutation(_) => context.schema.mutation_type().map(|t| t.name),
                OperationDefinition::Subscription(_) => {
                    context.schema.subscription_type().map(|t| t.name)
                }
            },
        };

        context.with_type(
            schema_type_name.map(|v| Type::NamedType(v)),
            |context| match definition {
                Definition::Fragment(fragment) => {
                    visit_fragment_definition(visitor, fragment, context, user_context)
                }
                Definition::Operation(operation) => {
                    visit_operation_definition(visitor, operation, context, user_context)
                }
            },
        );
    }
}

fn visit_directives<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    directives: &Vec<Directive>,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    for directive in directives {
        let directive_def_args = context
            .schema
            .directive_by_name(&directive.name)
            .map(|def| def.arguments);

        visitor.enter_directive(context, user_context, directive);
        visit_arguments(
            visitor,
            directive_def_args.as_ref(),
            &directive.arguments,
            context,
            user_context,
        );
        visitor.leave_directive(context, user_context, directive);
    }
}

fn visit_arguments<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    arguments_definition: Option<&Vec<schema::InputValue>>,
    arguments: &Vec<(String, Value)>,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    for argument in arguments {
        let arg_type = arguments_definition
            .and_then(|argument_defs| argument_defs.iter().find(|a| a.name.eq(&argument.0)))
            .map(|a| a.value_type.clone());

        context.with_input_type(arg_type, |context| {
            visitor.enter_argument(context, user_context, argument);
            visit_input_value(visitor, &argument.1, context, user_context);
            visitor.leave_argument(context, user_context, argument);
        })
    }
}

fn visit_input_value<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    input_value: &Value,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    match input_value {
        Value::Boolean(v) => {
            visitor.enter_scalar_value(context, user_context, v);
            visitor.leave_scalar_value(context, user_context, v);
        }
        Value::Float(v) => {
            visitor.enter_scalar_value(context, user_context, v);
            visitor.leave_scalar_value(context, user_context, v);
        }
        Value::Int(v) => {
            visitor.enter_scalar_value(context, user_context, v);
            visitor.leave_scalar_value(context, user_context, v);
        }
        Value::Null => {
            visitor.enter_null_value(context, user_context, ());
            visitor.leave_null_value(context, user_context, ());
        }
        Value::String(v) => {
            visitor.enter_scalar_value(context, user_context, v);
            visitor.leave_scalar_value(context, user_context, v);
        }
        Value::Enum(v) => {
            visitor.enter_enum_value(context, user_context, v.clone());
            visitor.leave_enum_value(context, user_context, v.clone());
        }
        Value::List(v) => {
            visitor.enter_list_value(context, user_context, v.clone());

            let input_type = context.current_input_type_literal().and_then(|t| match t {
                Type::ListType(inner_type) => Some(inner_type.as_ref().clone()),
                _ => None,
            });

            context.with_input_type(input_type, |context| {
                for item in v {
                    visit_input_value(visitor, item, context, user_context)
                }
            });

            visitor.leave_list_value(context, user_context, v.clone());
        }
        Value::Object(v) => {
            visitor.enter_object_value(context, user_context, v.clone());

            for (sub_key, sub_value) in v.iter() {
                let input_type = context
                    .current_input_type_literal()
                    .and_then(|v| context.schema.type_by_name(&v.inner_type()))
                    .and_then(|v| v.input_field_by_name(&sub_key))
                    .and_then(|v| Some(v.value_type));

                context.with_input_type(input_type, |context| {
                    let param = &(sub_key.clone(), sub_value.clone());
                    visitor.enter_object_field(context, user_context, param);
                    visit_input_value(visitor, sub_value, context, user_context);
                    visitor.leave_object_field(context, user_context, param);
                });
            }

            visitor.leave_object_value(context, user_context, v.clone());
        }
        Value::Variable(v) => {
            visitor.enter_variable_value(context, user_context, v.clone());
            visitor.leave_variable_value(context, user_context, v.clone());
        }
    }
}

fn visit_variable_definitions<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    variables: &[VariableDefinition],
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    for variable in variables {
        context.with_input_type(Some(variable.var_type.clone()), |context| {
            visitor.enter_variable_definition(context, user_context, variable);

            if let Some(default_value) = &variable.default_value {
                visit_input_value(visitor, &default_value, context, user_context);
            }

            // DOTAN: We should visit the directives as well here, but it's extracted in graphql_parser.

            visitor.leave_variable_definition(context, user_context, variable);
        })
    }
}

fn visit_selection<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    selection: &Selection,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    match selection {
        Selection::Field(field) => {
            let parent_type_def = context
                .current_parent_type()
                .and_then(|t| t.field_by_name(&field.name));

            let field_type = parent_type_def.clone().map(|f| f.field_type);
            let field_args = parent_type_def.map(|f| f.arguments);

            context.with_type(field_type, |context| {
                visitor.enter_field(context, user_context, field);
                visit_arguments(
                    visitor,
                    field_args.as_ref(),
                    &field.arguments,
                    context,
                    user_context,
                );
                visit_directives(visitor, &field.directives, context, user_context);
                visit_selection_set(visitor, &field.selection_set, context, user_context);
                visitor.leave_field(context, user_context, field);
            });
        }
        Selection::FragmentSpread(fragment_spread) => {
            visitor.enter_fragment_spread(context, user_context, fragment_spread);
            visit_directives(visitor, &fragment_spread.directives, context, user_context);
            visitor.leave_fragment_spread(context, user_context, fragment_spread);
        }
        Selection::InlineFragment(inline_fragment) => {
            if let Some(TypeCondition::On(fragment_condition)) = &inline_fragment.type_condition {
                context.with_type(
                    Some(Type::NamedType(fragment_condition.clone())),
                    |context| {
                        visitor.enter_inline_fragment(context, user_context, inline_fragment);
                        visit_directives(
                            visitor,
                            &inline_fragment.directives,
                            context,
                            user_context,
                        );
                        visit_selection_set(
                            visitor,
                            &inline_fragment.selection_set,
                            context,
                            user_context,
                        );
                        visitor.leave_inline_fragment(context, user_context, inline_fragment);
                    },
                );
            } else {
                visitor.enter_inline_fragment(context, user_context, inline_fragment);
                visit_directives(visitor, &inline_fragment.directives, context, user_context);
                visit_selection_set(
                    visitor,
                    &inline_fragment.selection_set,
                    context,
                    user_context,
                );
                visitor.leave_inline_fragment(context, user_context, inline_fragment);
            }
        }
    }
}

fn visit_selection_set<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    selection_set: &SelectionSet,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    context.with_parent_type(|context| {
        visitor.enter_selection_set(context, user_context, selection_set);

        for selection in &selection_set.items {
            visit_selection(visitor, selection, context, user_context);
        }

        visitor.leave_selection_set(context, user_context, selection_set);
    });
}

fn visit_fragment_definition<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    fragment: &FragmentDefinition,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    visitor.enter_fragment_definition(context, user_context, fragment);
    visit_directives(visitor, &fragment.directives, context, user_context);
    visit_selection_set(visitor, &fragment.selection_set, context, user_context);
    visitor.leave_fragment_definition(context, user_context, fragment);
}

fn visit_operation_definition<'a, Visitor, UserContext>(
    visitor: &mut Visitor,
    operation: &OperationDefinition,
    context: &mut OperationVisitorContext<'a>,
    user_context: &mut UserContext,
) where
    Visitor: OperationVisitor<'a, UserContext>,
{
    visitor.enter_operation_definition(context, user_context, operation);
    // DOTAN: Maybe we need to iterate directives as well? but i think graphql_parser does not have it at the moment?
    visit_variable_definitions(
        visitor,
        operation.variable_definitions(),
        context,
        user_context,
    );
    visit_selection_set(visitor, operation.selection_set(), context, user_context);
    visitor.leave_operation_definition(context, user_context, operation);
}

// Trait
pub trait OperationVisitor<'a, UserContext = ()> {
    fn enter_document(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &Document,
    ) {
    }
    fn leave_document(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &Document,
    ) {
    }

    fn enter_operation_definition(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &OperationDefinition,
    ) {
    }
    fn leave_operation_definition(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &OperationDefinition,
    ) {
    }

    fn enter_fragment_definition(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &FragmentDefinition,
    ) {
    }
    fn leave_fragment_definition(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &FragmentDefinition,
    ) {
    }

    fn enter_variable_definition(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &VariableDefinition,
    ) {
    }
    fn leave_variable_definition(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &VariableDefinition,
    ) {
    }

    fn enter_directive(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &Directive,
    ) {
    }
    fn leave_directive(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &Directive,
    ) {
    }

    fn enter_argument(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &(String, Value),
    ) {
    }
    fn leave_argument(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &(String, Value),
    ) {
    }

    fn enter_selection_set(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &SelectionSet,
    ) {
    }
    fn leave_selection_set(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &SelectionSet,
    ) {
    }

    fn enter_field(&mut self, _: &mut OperationVisitorContext<'a>, _: &mut UserContext, _: &Field) {
    }
    fn leave_field(&mut self, _: &mut OperationVisitorContext<'a>, _: &mut UserContext, _: &Field) {
    }

    fn enter_fragment_spread(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &FragmentSpread,
    ) {
    }
    fn leave_fragment_spread(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &FragmentSpread,
    ) {
    }

    fn enter_inline_fragment(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &InlineFragment,
    ) {
    }
    fn leave_inline_fragment(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &InlineFragment,
    ) {
    }

    fn enter_null_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: (),
    ) {
    }
    fn leave_null_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: (),
    ) {
    }

    fn enter_scalar_value<T>(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: T,
    ) {
    }
    fn leave_scalar_value<T>(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: T,
    ) {
    }

    fn enter_enum_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: String,
    ) {
    }
    fn leave_enum_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: String,
    ) {
    }

    fn enter_variable_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: String,
    ) {
    }
    fn leave_variable_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: String,
    ) {
    }

    fn enter_list_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: Vec<Value>,
    ) {
    }
    fn leave_list_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: Vec<Value>,
    ) {
    }

    fn enter_object_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: BTreeMap<String, Value>,
    ) {
    }
    fn leave_object_value(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: BTreeMap<String, Value>,
    ) {
    }

    fn enter_object_field(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &(String, Value),
    ) {
    }
    fn leave_object_field(
        &mut self,
        _: &mut OperationVisitorContext<'a>,
        _: &mut UserContext,
        _: &(String, Value),
    ) {
    }
}