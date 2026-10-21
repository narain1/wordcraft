//! Opt-in typed access to the bounded retained extended-property profile.
//! The ordinary reader/writer keep their existing best-effort behavior.
use std::sync::Arc;
use wordcraft_doc::Document;

use crate::package::{APP_KEPT, APP_KEPT_PROPS};

const MAX_VALUE_BYTES: usize = 2048;
const MAX_PROPERTIES: usize = 8;

/// Retained model values before the writer applies its privacy policy.
/// This is not an inventory of properties already omitted by the reader.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtendedProperties {
    pub template: Option<String>,
    pub manager: Option<String>,
    pub company: Option<String>,
    pub hyperlink_base: Option<String>,
    pub scale_crop: Option<bool>,
    pub links_up_to_date: Option<bool>,
    pub shared_doc: Option<bool>,
    pub hyperlinks_changed: Option<bool>,
}

/// One typed update. `None` clears; omitted properties remain untouched.
/// Empty text is a retained value, distinct from clearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtendedPropertyPatch {
    Template(Option<String>),
    Manager(Option<String>),
    Company(Option<String>),
    HyperlinkBase(Option<String>),
    ScaleCrop(Option<bool>),
    LinksUpToDate(Option<bool>),
    SharedDoc(Option<bool>),
    HyperlinksChanged(Option<bool>),
}

/// The strict typed profile cannot safely interpret or update retained state.
/// These errors do not diagnose properties already omitted during file reading.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtendedPropertiesError {
    #[error("extended properties exceed the bounded profile")]
    Limit,
    #[error("malformed retained extended-property record")]
    MalformedState,
    #[error("unsupported retained extended-property name")]
    UnsupportedProperty,
    #[error("duplicate retained extended property: {0}")]
    DuplicateProperty(&'static str),
    #[error("duplicate extended-property patch: {0}")]
    DuplicatePatch(&'static str),
    #[error("invalid extended-property value: {0}")]
    InvalidValue(&'static str),
}

type Result<T> = std::result::Result<T, ExtendedPropertiesError>;

#[derive(Clone, Copy)]
struct Entry<'a> {
    index: usize,
    value: &'a str,
}

#[derive(Default)]
struct Retained<'a> {
    entries: [Option<Entry<'a>>; MAX_PROPERTIES],
}

impl<'a> Retained<'a> {
    fn value(&self, index: usize) -> Option<&'a str> {
        self.entries.iter().flatten().find(|entry| entry.index == index).map(|entry| entry.value)
    }
}

fn name(index: usize) -> Result<&'static str> {
    APP_KEPT_PROPS.get(index).copied().ok_or(ExtendedPropertiesError::MalformedState)
}

fn max_retained_bytes() -> Result<usize> {
    APP_KEPT_PROPS.iter().try_fold(0usize, |total, name| {
        total.checked_add(name.len()).and_then(|n| n.checked_add(MAX_VALUE_BYTES + 2)).ok_or(ExtendedPropertiesError::Limit)
    })
}

fn boolean(value: &str, index: usize) -> Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(ExtendedPropertiesError::InvalidValue(name(index)?)),
    }
}

fn validate_text(value: &str, index: usize) -> Result<()> {
    if value.len() > MAX_VALUE_BYTES {
        return Err(ExtendedPropertiesError::Limit);
    }
    if value.chars().any(|c| c.is_control() || !crate::xml::xml_char_ok(c)) {
        return Err(ExtendedPropertiesError::InvalidValue(name(index)?));
    }
    Ok(())
}

/// No heap allocation: bounds, UTF8, separators, names, uniqueness and scalars
/// are checked before a public snapshot or replacement buffer is constructed.
fn retained(document: &Document) -> Result<Retained<'_>> {
    let mut parsed = Retained::default();
    let Some(bytes) = document.passthrough.get(APP_KEPT) else { return Ok(parsed) };
    if bytes.len() > max_retained_bytes()? {
        return Err(ExtendedPropertiesError::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ExtendedPropertiesError::MalformedState)?;
    for (count, line) in text.split_terminator('\n').enumerate() {
        if count >= MAX_PROPERTIES {
            return Err(ExtendedPropertiesError::Limit);
        }
        let (key, value) = line.split_once('\t').ok_or(ExtendedPropertiesError::MalformedState)?;
        let index = APP_KEPT_PROPS.iter().position(|candidate| *candidate == key).ok_or(ExtendedPropertiesError::UnsupportedProperty)?;
        if parsed.value(index).is_some() {
            return Err(ExtendedPropertiesError::DuplicateProperty(name(index)?));
        }
        validate_text(value, index)?;
        if index >= 4 {
            boolean(value, index)?;
        }
        *parsed.entries.get_mut(count).ok_or(ExtendedPropertiesError::Limit)? = Some(Entry { index, value });
    }
    Ok(parsed)
}

/// Return an owned typed snapshot of at most eight unique retained properties.
/// Text is at most 2048 UTF8 bytes, contains no control characters and must be
/// representable by the existing native XML writer. Retained
/// booleans accept exactly `true`, `false`, `1`, or `0`. Ambiguous/invalid state
/// is an error; legacy reader omissions cannot be recovered by this function.
pub fn extended_properties(document: &Document) -> Result<ExtendedProperties> {
    let parsed = retained(document)?;
    let text = |index| parsed.value(index).map(str::to_owned);
    let flag = |index| parsed.value(index).map(|value| boolean(value, index)).transpose();
    Ok(ExtendedProperties {
        template: text(0),
        manager: text(1),
        company: text(2),
        hyperlink_base: text(3),
        scale_crop: flag(4)?,
        links_up_to_date: flag(5)?,
        shared_doc: flag(6)?,
        hyperlinks_changed: flag(7)?,
    })
}

#[derive(Clone, Copy)]
enum Value<'a> {
    Text(Option<&'a str>),
    Boolean(Option<bool>),
}

impl ExtendedPropertyPatch {
    fn borrowed(&self) -> (usize, Value<'_>) {
        match self {
            Self::Template(value) => (0, Value::Text(value.as_deref())),
            Self::Manager(value) => (1, Value::Text(value.as_deref())),
            Self::Company(value) => (2, Value::Text(value.as_deref())),
            Self::HyperlinkBase(value) => (3, Value::Text(value.as_deref())),
            Self::ScaleCrop(value) => (4, Value::Boolean(*value)),
            Self::LinksUpToDate(value) => (5, Value::Boolean(*value)),
            Self::SharedDoc(value) => (6, Value::Boolean(*value)),
            Self::HyperlinksChanged(value) => (7, Value::Boolean(*value)),
        }
    }
}

fn replacement<'a>(current: Option<&'a str>, patch: Option<Value<'a>>, index: usize) -> Result<Option<&'a str>> {
    match patch {
        None => Ok(current),
        Some(Value::Text(value)) => Ok(value),
        Some(Value::Boolean(None)) => Ok(None),
        Some(Value::Boolean(Some(value))) => {
            if current.map(|text| boolean(text, index)).transpose()? == Some(value) {
                Ok(current) // Preserve an unchanged 0/1 lexical form.
            } else {
                Ok(Some(if value { "true" } else { "false" }))
            }
        }
    }
}

/// Atomically update the strict retained profile using at most eight unique patches.
/// Validate the complete current record even for an empty, clearing or semantic
/// no-op patch. Untouched fields keep order/lexical values; semantic no-ops retain
/// the original bytes and Arc. New fields append in the native property order.
/// Changed booleans use `true`/`false`; clearing the last field removes only the
/// retained-property record. Other model state and default writer policy are unchanged.
pub fn patch_extended_properties(document: &mut Document, patches: &[ExtendedPropertyPatch]) -> Result<()> {
    if patches.len() > MAX_PROPERTIES {
        return Err(ExtendedPropertiesError::Limit);
    }
    let mut changes = [None; MAX_PROPERTIES];
    for patch in patches {
        let (index, value) = patch.borrowed();
        let slot = changes.get_mut(index).ok_or(ExtendedPropertiesError::MalformedState)?;
        if slot.is_some() {
            return Err(ExtendedPropertiesError::DuplicatePatch(name(index)?));
        }
        if let Value::Text(Some(text)) = value {
            validate_text(text, index)?;
        }
        *slot = Some(value);
    }
    let parsed = retained(document)?;
    let mut changed = false;
    let mut output = Retained::default();
    let mut count = 0usize;
    for entry in parsed.entries.iter().flatten() {
        let value = replacement(Some(entry.value), changes.get(entry.index).copied().flatten(), entry.index)?;
        changed |= value != Some(entry.value);
        if let Some(value) = value {
            *output.entries.get_mut(count).ok_or(ExtendedPropertiesError::Limit)? = Some(Entry { index: entry.index, value });
            count += 1;
        }
    }
    for (index, patch) in changes.into_iter().enumerate() {
        if parsed.value(index).is_none()
            && let Some(value) = replacement(None, patch, index)?
        {
            *output.entries.get_mut(count).ok_or(ExtendedPropertiesError::Limit)? = Some(Entry { index, value });
            count += 1;
            changed = true;
        }
    }
    if !changed {
        return Ok(());
    }
    let bytes = output.entries.iter().flatten().try_fold(0usize, |total, entry| {
        total
            .checked_add(name(entry.index)?.len())
            .and_then(|n| n.checked_add(entry.value.len()))
            .and_then(|n| n.checked_add(2))
            .ok_or(ExtendedPropertiesError::Limit)
    })?;
    if bytes > max_retained_bytes()? {
        return Err(ExtendedPropertiesError::Limit);
    }
    let mut result = String::with_capacity(bytes);
    for entry in output.entries.iter().flatten() {
        result.push_str(name(entry.index)?);
        result.push('\t');
        result.push_str(entry.value);
        result.push('\n');
    }
    if result.is_empty() {
        document.passthrough.remove(APP_KEPT);
    } else {
        document.passthrough.insert(APP_KEPT.into(), Arc::new(result.into_bytes()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "extended_properties_tests.rs"]
mod tests;
