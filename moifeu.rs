//! MOIFEU-PACK v2 — Full Rust Protocol Wrapper
//!
//! A comprehensive wrapper for the Anaseos-Moifeu protocol,
//! providing parsing, construction, validation, SpacetimeDB integration,
//! and Zed agent compatibility.
//!
//! ## Overview
//!
//! Moifeu (Motif-Efficient Universal) is a token-dense communication protocol
//! for LLM-to-LLM and human-to-LLM communication.
//!
//! ## Wire Format
//!
//! ```text
//! [τ1τ2φκαdαsαc]|[body]
//!
//! Where:
//! - τ1, τ2: Ternary operation digits (0-2)
//! - φ: Phase rotation (0-3)
//! - κ: Colour gradient (b|g|y|r|w)
//! - αd, αs, αc: Analogue values (0-9) for depth, speed, certainty
//! ```
//!
//! ## Zed Integration
//!
//! Moifeu integrates with Zed agents through SpacetimeDB's gradient_map procedure,
//! which provides color gradient maps as the data communication layer.
//! Token buckets are managed via the yield system (YieldTarget, YieldLedger).
//!
//! ## Color Gradient Maps
//!
//! The `GradientCell` struct represents the fog field data from SpacetimeDB,
//! containing density, stance, angle, hue, saturation, and whisper for each seat.

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cmp::max;
use std::str::FromStr;
use std::sync::RwLock;

// Re-export gradient codec for Moifeu-based gradient encoding
pub mod moifeu_gradient;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Comprehensive error type for Moifeu parsing and validation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoifeuError {
    NoHeaderBodySeparator,
    InvalidHeaderLength(usize),
    InvalidTernaryDigit(char, &'static str),
    InvalidPhaseDigit(char),
    InvalidKappa(char),
    InvalidAnalogueDigit(char, &'static str),
    UnknownOperation(String),
    UnknownModifier(String),
    UnknownLanguage(String),
    InvalidDirection(String),
    UnrecognizedToken(String),
    NoOperation,
    CertaintyOverflow(u8),
    AbsolutePriorityReservedForStream,
    ValidationErrors(Vec<MoifeuError>),
    ParseError(String),
}

impl std::fmt::Display for MoifeuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoifeuError::NoHeaderBodySeparator => write!(f, "no header|body separator"),
            MoifeuError::InvalidHeaderLength(len) => {
                write!(f, "header must be 7 chars, got {}", len)
            }
            MoifeuError::InvalidTernaryDigit(c, field) => {
                write!(f, "{} must be ternary (0-2), got '{}'", field, c)
            }
            MoifeuError::InvalidPhaseDigit(c) => write!(f, "phase must be 0-3, got '{}'", c),
            MoifeuError::InvalidKappa(c) => write!(f, "kappa must be b|g|y|r|w, got '{}'", c),
            MoifeuError::InvalidAnalogueDigit(c, field) => {
                write!(f, "{} must be 0-9, got '{}'", field, c)
            }
            MoifeuError::UnknownOperation(op) => write!(f, "unknown operation: {}", op),
            MoifeuError::UnknownModifier(r_mod) => write!(f, "unknown modifier: {}", r_mod),
            MoifeuError::UnknownLanguage(lang) => write!(f, "unknown language: {}", lang),
            MoifeuError::InvalidDirection(dir) => write!(f, "invalid direction: {}", dir),
            MoifeuError::UnrecognizedToken(tok) => write!(f, "unrecognized token: '{}'", tok),
            MoifeuError::NoOperation => write!(f, "beam has no operation"),
            MoifeuError::CertaintyOverflow(v) => write!(f, "certainty overflow: {}", v),
            MoifeuError::AbsolutePriorityReservedForStream => {
                write!(f, "w priority reserved for stream class")
            }
            MoifeuError::ValidationErrors(errs) => {
                write!(f, "validation errors: ")?;
                for (i, e) in errs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", e)?;
                }
                Ok(())
            }
            MoifeuError::ParseError(msg) => write!(f, "parse error: {}", msg),
        }
    }
}

impl std::error::Error for MoifeuError {}

impl From<String> for MoifeuError {
    fn from(s: String) -> Self {
        MoifeuError::ParseError(s)
    }
}

impl From<&str> for MoifeuError {
    fn from(s: &str) -> Self {
        MoifeuError::ParseError(s.to_string())
    }
}

impl MoifeuError {
    pub fn validation_errors(errors: Vec<MoifeuError>) -> Option<Self> {
        if errors.is_empty() {
            None
        } else if errors.len() == 1 {
            Some(errors.into_iter().next().unwrap())
        } else {
            Some(MoifeuError::ValidationErrors(errors))
        }
    }
}

/// Result type for Moifeu operations
pub type ParseResult<T> = Result<T, MoifeuError>;

// ============================================================================
// CONSTANTS
// ============================================================================

const OPS: &[&str] = &[
    "tr", "sm", "an", "gen", "ex", "cl", "cmp", "fix", "exp", "fmt", "chk", "cvt", "red",
    "GRAD_START", "GRAD_END", "CHECKSUM", "DATA", "GRADIENT",
];
const MODS: &[&str] = &["brf", "det", "fml", "inf", "json", "txt", "code", "tbl"];
const LANGS: &[&str] = &[
    "en", "hi", "ta", "bn", "te", "mr", "gu", "kn", "ml", "pa", "or", "sa", "ur", "auto",
];

// ============================================================================
// MAIN BEAM STRUCTURE
// ============================================================================

/// A parsed Moifeu beam with Anaseos dimensional header and Moifeu grammar body
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoifeuBeam {
    pub op_class: u8,
    pub op_level: u8,
    pub phase: u8,
    pub colour: char,
    pub depth: u8,
    pub speed: u8,
    pub certainty: u8,
    pub context: Option<String>,
    pub op: Option<String>,
    pub ops: Vec<String>,
    pub src_lang: Option<String>,
    pub tgt_lang: Option<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub modifiers: Vec<String>,
    pub constraints: Vec<String>,
    pub flags: Vec<char>,
    pub raw: String,
}

impl Default for MoifeuBeam {
    fn default() -> Self {
        MoifeuBeam {
            op_class: 0,
            op_level: 1,
            phase: 2,
            colour: 'g',
            depth: 5,
            speed: 5,
            certainty: 5,
            context: None,
            op: None,
            ops: Vec::new(),
            src_lang: None,
            tgt_lang: None,
            input: None,
            output: None,
            modifiers: Vec::new(),
            constraints: Vec::new(),
            flags: Vec::new(),
            raw: String::new(),
        }
    }
}

// ============================================================================
// PARSING
// ============================================================================

fn split_beam(raw: &str) -> ParseResult<(&str, &str)> {
    match raw.split_once('|') {
        Some((h, b)) => Ok((h.trim(), b.trim())),
        None => Err(MoifeuError::NoHeaderBodySeparator),
    }
}

fn parse_header(h: &str) -> ParseResult<(u8, u8, u8, char, u8, u8, u8)> {
    let bytes: Vec<char> = h.chars().collect();
    if bytes.len() != 7 {
        return Err(MoifeuError::InvalidHeaderLength(bytes.len()));
    }
    let t1 = bytes[0]
        .to_digit(10)
        .ok_or_else(|| MoifeuError::InvalidTernaryDigit(bytes[0], "tau1"))? as u8;
    if t1 > 2 {
        return Err(MoifeuError::InvalidTernaryDigit(bytes[0], "tau1"));
    }
    let t2 = bytes[1]
        .to_digit(10)
        .ok_or_else(|| MoifeuError::InvalidTernaryDigit(bytes[1], "tau2"))? as u8;
    if t2 > 2 {
        return Err(MoifeuError::InvalidTernaryDigit(bytes[1], "tau2"));
    }
    let phi = bytes[2]
        .to_digit(10)
        .ok_or_else(|| MoifeuError::InvalidPhaseDigit(bytes[2]))? as u8;
    if phi > 3 {
        return Err(MoifeuError::InvalidPhaseDigit(bytes[2]));
    }
    let kappa = bytes[3];
    if !matches!(kappa, 'b' | 'g' | 'y' | 'r' | 'w') {
        return Err(MoifeuError::InvalidKappa(kappa));
    }
    let d = bytes[4]
        .to_digit(10)
        .ok_or_else(|| MoifeuError::InvalidAnalogueDigit(bytes[4], "depth"))? as u8;
    if d > 9 {
        return Err(MoifeuError::InvalidAnalogueDigit(bytes[4], "depth"));
    }
    let s = bytes[5]
        .to_digit(10)
        .ok_or_else(|| MoifeuError::InvalidAnalogueDigit(bytes[5], "speed"))? as u8;
    if s > 9 {
        return Err(MoifeuError::InvalidAnalogueDigit(bytes[5], "speed"));
    }
    let c = bytes[6]
        .to_digit(10)
        .ok_or_else(|| MoifeuError::InvalidAnalogueDigit(bytes[6], "certainty"))? as u8;
    if c > 9 {
        return Err(MoifeuError::InvalidAnalogueDigit(bytes[6], "certainty"));
    }
    Ok((t1, t2, phi, kappa, d, s, c))
}

fn parse_body(
    b: &str,
) -> ParseResult<(
    Option<String>,
    Option<String>,
    Vec<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Vec<char>,
)> {
    let mut context = None;
    let mut op = None;
    let mut ops: Vec<String> = Vec::new();
    let mut src_lang = None;
    let mut tgt_lang = None;
    let mut input = None;
    let mut output = None;
    let mut modifiers = Vec::new();
    let mut constraints = Vec::new();
    let mut flags = Vec::new();
    let mut rest = b;
    if let Some(r) = rest.strip_prefix('@') {
        let end = r.find(|c: char| c.is_whitespace()).unwrap_or(r.len());
        context = Some(r[..end].to_string());
        rest = r[end..].trim_start();
    }
    loop {
        if let Some(r) = rest.strip_prefix('!') {
            flags.push('!');
            rest = r.trim_start();
        } else if let Some(r) = rest.strip_prefix('~') {
            flags.push('~');
            rest = r.trim_start();
        } else if let Some(r) = rest.strip_prefix('*') {
            flags.push('*');
            rest = r.trim_start();
        } else {
            break;
        }
    }
    for tok in rest.split_whitespace() {
        if let Some(r) = tok.strip_prefix('+') {
            if MODS.contains(&r) {
                modifiers.push(r.to_string());
            } else {
                return Err(MoifeuError::UnknownModifier(r.to_string()));
            }
        } else if let Some(r) = tok.strip_prefix('#') {
            if !r.is_empty() {
                constraints.push(r.to_string());
            }
        } else if let Some(r) = tok.strip_prefix('<') {
            output = Some(r.to_string());
        } else if tok.contains('>') && !tok.starts_with('<') && !tok.starts_with('>') {
            let parts: Vec<&str> = tok.split('>').collect();
            if parts.len() == 2 && LANGS.contains(&parts[0]) && LANGS.contains(&parts[1]) {
                src_lang = Some(parts[0].to_string());
                tgt_lang = Some(parts[1].to_string());
            } else {
                return Err(MoifeuError::InvalidDirection(tok.to_string()));
            }
        } else if let Some(r) = tok.strip_prefix('>') {
            if let Some((s, t)) = r.split_once('>') {
                if !LANGS.contains(&s) || !LANGS.contains(&t) {
                    return Err(MoifeuError::InvalidDirection(tok.to_string()));
                }
                src_lang = Some(s.to_string());
                tgt_lang = Some(t.to_string());
            } else {
                input = Some(r.to_string());
            }
        } else if let Some(r) = tok.strip_prefix('?') {
            if op.is_none() {
                op = Some("an".to_string());
                ops.push("an".to_string());
            }
            input = Some(format!("?{}", r));
        } else if tok == "!" || tok == "~" || tok == "*" {
            flags.push(tok.chars().next().unwrap());
        } else if OPS.contains(&tok) {
            if op.is_none() {
                op = Some(tok.to_string());
            }
            ops.push(tok.to_string());
        } else if LANGS.contains(&tok) {
            if src_lang.is_none() {
                src_lang = Some(tok.to_string());
            }
        } else {
            return Err(MoifeuError::UnrecognizedToken(tok.to_string()));
        }
    }
    Ok((
        context,
        op,
        ops,
        src_lang,
        tgt_lang,
        input,
        output,
        modifiers,
        constraints,
        flags,
    ))
}

/// Parse a Moifeu beam from its wire format string
pub fn parse_beam(raw: &str) -> ParseResult<MoifeuBeam> {
    let (h, b) = split_beam(raw)?;
    let (op_class, op_level, phase, colour, depth, speed, certainty) = parse_header(h)?;
    let (context, op, ops, src_lang, tgt_lang, input, output, modifiers, constraints, flags) =
        parse_body(b)?;
    Ok(MoifeuBeam {
        op_class,
        op_level,
        phase,
        colour,
        depth,
        speed,
        certainty,
        context,
        op,
        ops,
        src_lang,
        tgt_lang,
        input,
        output,
        modifiers,
        constraints,
        flags,
        raw: raw.to_string(),
    })
}

/// Emit a beam back to canonical wire form (round-trip check)
pub fn emit_beam(b: &MoifeuBeam) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{}{}{}{}{}{}{}",
        b.op_class, b.op_level, b.phase, b.colour, b.depth, b.speed, b.certainty
    ));
    s.push('|');
    if let Some(c) = &b.context {
        s.push('@');
        s.push_str(c);
        s.push(' ');
    }
    for f in &b.flags {
        s.push(*f);
    }
    if !b.flags.is_empty() {
        s.push(' ');
    }
    for (i, op) in b.ops.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(op);
    }
    if !b.ops.is_empty() {
        s.push(' ');
    }
    if let (Some(src), Some(tgt)) = (&b.src_lang, &b.tgt_lang) {
        s.push_str(src);
        s.push('>');
        s.push_str(tgt);
        s.push(' ');
    } else if let Some(src) = &b.src_lang {
        s.push_str(src);
        s.push(' ');
    }
    if let Some(i) = &b.input {
        s.push('>');
        s.push_str(i);
        s.push(' ');
    }
    if let Some(o) = &b.output {
        s.push('<');
        s.push_str(o);
        s.push(' ');
    }
    for m in &b.modifiers {
        s.push('+');
        s.push_str(m);
        s.push(' ');
    }
    for c in &b.constraints {
        s.push('#');
        s.push_str(c);
        s.push(' ');
    }
    s.trim_end().to_string()
}

/// Validate a beam against Pantheon law
pub fn beam_is_actionable(b: &MoifeuBeam) -> Result<(), String> {
    if b.op.is_none() {
        return Err("beam has no operation".into());
    }
    if b.certainty > 9 {
        return Err("certainty overflow".into());
    }
    if b.colour == 'w' && b.op_class != 2 {
        return Err("w (absolute) priority reserved for stream class".into());
    }
    Ok(())
}

// ============================================================================
// FROM/TRYFROM TRAITS
// ============================================================================

impl FromStr for MoifeuBeam {
    type Err = MoifeuError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_beam(s)
    }
}

impl TryFrom<&str> for MoifeuBeam {
    type Error = MoifeuError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        parse_beam(value)
    }
}

impl TryFrom<String> for MoifeuBeam {
    type Error = MoifeuError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        parse_beam(&value)
    }
}

// ============================================================================
// VALIDATION
// ============================================================================

impl MoifeuBeam {
    pub fn validate(&self) -> ParseResult<()> {
        let mut errors = Vec::new();
        if self.op.is_none() {
            errors.push(MoifeuError::NoOperation);
        }
        if self.certainty > 9 {
            errors.push(MoifeuError::CertaintyOverflow(self.certainty));
        }
        if self.colour == 'w' && self.op_class != 2 {
            errors.push(MoifeuError::AbsolutePriorityReservedForStream);
        }
        for op in &self.ops {
            if !OPS.contains(&op.as_str()) {
                errors.push(MoifeuError::UnknownOperation(op.clone()));
            }
        }
        for r_mod in &self.modifiers {
            if !MODS.contains(&r_mod.as_str()) {
                errors.push(MoifeuError::UnknownModifier(r_mod.clone()));
            }
        }
        if let Some(lang) = &self.src_lang {
            if !LANGS.contains(&lang.as_str()) {
                errors.push(MoifeuError::UnknownLanguage(lang.clone()));
            }
        }
        if let Some(lang) = &self.tgt_lang {
            if !LANGS.contains(&lang.as_str()) {
                errors.push(MoifeuError::UnknownLanguage(lang.clone()));
            }
        }
        match MoifeuError::validation_errors(errors) {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }
    pub fn validation_errors(&self) -> Vec<MoifeuError> {
        let mut errors = Vec::new();
        if self.op.is_none() {
            errors.push(MoifeuError::NoOperation);
        }
        if self.certainty > 9 {
            errors.push(MoifeuError::CertaintyOverflow(self.certainty));
        }
        if self.colour == 'w' && self.op_class != 2 {
            errors.push(MoifeuError::AbsolutePriorityReservedForStream);
        }
        for op in &self.ops {
            if !OPS.contains(&op.as_str()) {
                errors.push(MoifeuError::UnknownOperation(op.clone()));
            }
        }
        for r_mod in &self.modifiers {
            if !MODS.contains(&r_mod.as_str()) {
                errors.push(MoifeuError::UnknownModifier(r_mod.clone()));
            }
        }
        if let Some(lang) = &self.src_lang {
            if !LANGS.contains(&lang.as_str()) {
                errors.push(MoifeuError::UnknownLanguage(lang.clone()));
            }
        }
        if let Some(lang) = &self.tgt_lang {
            if !LANGS.contains(&lang.as_str()) {
                errors.push(MoifeuError::UnknownLanguage(lang.clone()));
            }
        }
        errors
    }
}

// ============================================================================
// HELPER METHODS
// ============================================================================

impl MoifeuBeam {
    pub fn operation_type(&self) -> &'static str {
        match (self.op_class, self.op_level) {
            (0, 0) => "query_low",
            (0, 1) => "query_std",
            (0, 2) => "query_urgent",
            (1, 0) => "transform_low",
            (1, 1) => "transform_std",
            (1, 2) => "transform_urgent",
            (2, 0) => "stream_low",
            (2, 1) => "stream_std",
            (2, 2) => "stream_urgent",
            _ => "unknown",
        }
    }
    pub fn is_query(&self) -> bool {
        self.op_class == 0
    }
    pub fn is_transform(&self) -> bool {
        self.op_class == 1
    }
    pub fn is_stream(&self) -> bool {
        self.op_class == 2
    }
    pub fn phase_name(&self) -> &'static str {
        match self.phase {
            0 => "input",
            1 => "processing",
            2 => "output",
            3 => "feedback",
            _ => "unknown",
        }
    }
    pub fn colour_name(&self) -> &'static str {
        match self.colour {
            'b' => "cold",
            'g' => "warm",
            'y' => "hot",
            'r' => "critical",
            'w' => "absolute",
            _ => "unknown",
        }
    }
    pub fn priority_weight(&self) -> f32 {
        match self.colour {
            'b' => 0.0,
            'g' => 0.25,
            'y' => 0.50,
            'r' => 0.75,
            'w' => 1.0,
            _ => 0.25,
        }
    }
    pub fn is_high_priority(&self) -> bool {
        self.colour == 'r' || self.colour == 'w'
    }
    pub fn is_urgent(&self) -> bool {
        self.op_level == 2 || self.is_high_priority()
    }
    pub fn to_spacetime_push_args(&self) -> (u8, f32, i8, String) {
        let seat_id = self
            .context
            .as_deref()
            .and_then(|c| c.parse::<u8>().ok())
            .unwrap_or(0);
        let density = self.depth as f32 / 10.0 * 9.0;
        let stance = match self.colour {
            'b' => -1,
            'g' => 0,
            'y' | 'r' | 'w' => 1,
            _ => 0,
        };
        let whisper = self.input.clone().unwrap_or_default();
        (seat_id, density, stance, whisper)
    }
    pub fn to_kappa(&self) -> String {
        self.colour.to_string()
    }
}

// ============================================================================
// BEAM BUILDER
// ============================================================================

#[derive(Debug, Clone)]
pub struct MoifeuBeamBuilder {
    beam: MoifeuBeam,
}

impl MoifeuBeamBuilder {
    pub fn new() -> Self {
        MoifeuBeamBuilder {
            beam: MoifeuBeam::default(),
        }
    }
    pub fn from_beam(beam: MoifeuBeam) -> Self {
        MoifeuBeamBuilder { beam }
    }
    pub fn header(mut self, op_class: u8, op_level: u8, phase: u8) -> Self {
        self.beam.op_class = op_class;
        self.beam.op_level = op_level;
        self.beam.phase = phase;
        self
    }
    pub fn op_class(mut self, v: u8) -> Self {
        self.beam.op_class = v;
        self
    }
    pub fn op_level(mut self, v: u8) -> Self {
        self.beam.op_level = v;
        self
    }
    pub fn phase(mut self, v: u8) -> Self {
        self.beam.phase = v;
        self
    }
    pub fn kappa(mut self, v: char) -> Self {
        self.beam.colour = v;
        self
    }
    pub fn analogue(mut self, depth: u8, speed: u8, certainty: u8) -> Self {
        self.beam.depth = depth;
        self.beam.speed = speed;
        self.beam.certainty = certainty;
        self
    }
    pub fn context(mut self, v: impl Into<String>) -> Self {
        self.beam.context = Some(v.into());
        self
    }
    pub fn operation(mut self, v: impl Into<String>) -> Self {
        let s = v.into();
        self.beam.op = Some(s.clone());
        if !self.beam.ops.contains(&s) {
            self.beam.ops.push(s);
        }
        self
    }
    pub fn language(mut self, src: impl Into<String>, tgt: impl Into<String>) -> Self {
        self.beam.src_lang = Some(src.into());
        self.beam.tgt_lang = Some(tgt.into());
        self
    }
    pub fn input(mut self, v: impl Into<String>) -> Self {
        self.beam.input = Some(v.into());
        self
    }
    pub fn flag(mut self, v: char) -> Self {
        self.beam.flags.push(v);
        self
    }
    pub fn as_query(mut self) -> Self {
        self.beam.op_class = 0;
        self
    }
    pub fn as_transform(mut self) -> Self {
        self.beam.op_class = 1;
        self
    }
    pub fn as_stream(mut self) -> Self {
        self.beam.op_class = 2;
        self
    }
    pub fn build(self) -> ParseResult<MoifeuBeam> {
        let beam = self.beam;
        beam.validate()?;
        Ok(beam)
    }
}

impl Default for MoifeuBeamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MACROS
// ============================================================================

#[macro_export]
macro_rules! moifeu {
    ($class:expr, $level:expr, $phase:expr, $kappa:expr, $depth:expr, $speed:expr, $certainty:expr, $op:expr, $input:expr) => {
        MoifeuBeamBuilder::new()
            .header($class, $level, $phase)
            .kappa($kappa)
            .analogue($depth, $speed, $certainty)
            .operation($op)
            .input($input)
            .build()
            .expect("moifeu build failed")
    };
    ($kappa:expr, $op:expr, $input:expr) => {
        moifeu!(1, 1, 2, $kappa, 5, 5, 5, $op, $input)
    };
    (query: $input:expr) => {
        moifeu!(0, 1, 2, 'g', 5, 5, 5, "an", $input)
    };
    (transform: $op:expr, $input:expr) => {
        moifeu!(1, 1, 2, 'g', 5, 5, 5, $op, $input)
    };
    (urgent: $op:expr, $input:expr) => {
        moifeu!(1, 2, 2, 'r', 9, 9, 9, $op, $input)
    };
    (stream: $op:expr, $input:expr) => {
        moifeu!(2, 1, 2, 'w', 5, 5, 5, $op, $input)
    };
}

// ============================================================================
// BACKWARD COMPATIBILITY
// ============================================================================

/// Backward compatible parse_beam that returns String errors
pub fn parse_beam_compat(raw: &str) -> Result<MoifeuBeam, String> {
    parse_beam(raw).map_err(|e| e.to_string())
}

/// Backward compatible beam_is_actionable
pub fn beam_is_actionable_compat(b: &MoifeuBeam) -> Result<(), String> {
    beam_is_actionable(b)
}

// ============================================================================
// ASYNC STREAM PARSING
// ============================================================================

#[cfg(feature = "async")]
pub mod async_stream {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::sync::mpsc;

    /// Async parser for streaming beams
    ///
    /// Reads lines from an async reader, parses each as a Moifeu beam,
    /// and sends results to the provided channel.
    ///
    /// # Example
    /// ```ignore
    /// use tokio::sync::mpsc;
    /// use std::io::Cursor;
    ///
    /// let (tx, mut rx) = mpsc::channel(32);
    /// let input = Cursor::new("112g435|tr hi>en\n200r999|an test\n");
    ///
    /// tokio::spawn(async_stream::parse_beam_stream(input, tx));
    ///
    /// while let Some(result) = rx.recv().await {
    ///     match result {
    ///         Ok(beam) => println!("Parsed: {:?}", beam),
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// ```
    pub async fn parse_beam_stream<R>(
        reader: R,
        sender: mpsc::Sender<ParseResult<MoifeuBeam>>,
    ) -> ParseResult<()>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut reader = BufReader::new(reader);
        let mut buffer = String::with_capacity(512);

        loop {
            buffer.clear();
            let bytes = reader
                .read_line(&mut buffer)
                .await
                .map_err(|e| MoifeuError::ParseError(e.to_string()))?;

            if bytes == 0 {
                break; // EOF
            }

            let beam = parse_beam(buffer.trim());
            sender
                .send(beam)
                .await
                .map_err(|e| MoifeuError::ParseError(e.to_string()))?;
        }

        Ok(())
    }

    /// Parse beams from a byte stream with custom delimiter
    pub async fn parse_beam_stream_delim<R>(
        reader: R,
        sender: mpsc::Sender<ParseResult<MoifeuBeam>>,
        delimiter: u8,
    ) -> ParseResult<()>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        use tokio::io::AsyncReadExt;

        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::with_capacity(512);

        loop {
            buffer.clear();
            let bytes_read = reader
                .read_until(delimiter, &mut buffer)
                .await
                .map_err(|e| MoifeuError::ParseError(e.to_string()))?;

            if bytes_read == 0 {
                break; // EOF
            }

            let raw = String::from_utf8_lossy(&buffer[..bytes_read]);
            let beam = parse_beam(raw.trim());
            sender
                .send(beam)
                .await
                .map_err(|e| MoifeuError::ParseError(e.to_string()))?;
        }

        Ok(())
    }
}

// ============================================================================
// BEAM BATCH
// ============================================================================

/// A collection of Moifeu beams for batch processing
///
/// Useful for batch operations, priority sorting, and bulk processing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoifeuBeamBatch {
    beams: Vec<MoifeuBeam>,
}

impl MoifeuBeamBatch {
    /// Create a new empty batch
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a batch with initial capacity
    pub fn with_capacity(cap: usize) -> Self {
        MoifeuBeamBatch {
            beams: Vec::with_capacity(cap),
        }
    }

    /// Add a beam to the batch
    pub fn add(&mut self, beam: MoifeuBeam) {
        self.beams.push(beam);
    }

    /// Add multiple beams from an iterator
    pub fn extend(&mut self, beams: impl IntoIterator<Item = MoifeuBeam>) {
        self.beams.extend(beams);
    }

    /// Get all beams as a slice
    pub fn beams(&self) -> &[MoifeuBeam] {
        &self.beams
    }

    /// Consume the batch and return all beams
    pub fn into_beams(self) -> Vec<MoifeuBeam> {
        self.beams
    }

    /// Get the number of beams in the batch
    pub fn len(&self) -> usize {
        self.beams.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.beams.is_empty()
    }

    /// Sort beams by priority (highest first)
    ///
    /// Beams with higher priority_weight() come first.
    /// For beams with equal priority, maintains original order (stable sort).
    pub fn sort_by_priority(&mut self) {
        self.beams.sort_by(|a, b| {
            b.priority_weight()
                .partial_cmp(&a.priority_weight())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Sort by priority, then by operation class
    pub fn sort_by_priority_and_class(&mut self) {
        self.beams.sort_by(|a, b| {
            match b.priority_weight().partial_cmp(&a.priority_weight()) {
                Some(std::cmp::Ordering::Equal) => {
                    // Same priority: sort by class (stream > transform > query)
                    b.op_class.cmp(&a.op_class)
                }
                Some(other) => other,
                None => std::cmp::Ordering::Equal,
            }
        });
    }

    /// Get total priority weight of all beams
    pub fn total_priority(&self) -> f32 {
        self.beams.iter().map(|b| b.priority_weight()).sum()
    }

    /// Get average priority weight
    pub fn avg_priority(&self) -> f32 {
        if self.beams.is_empty() {
            0.0
        } else {
            self.total_priority() / self.beams.len() as f32
        }
    }

    /// Count beams by operation class
    ///
    /// Returns: (query_count, transform_count, stream_count)
    pub fn count_by_class(&self) -> (usize, usize, usize) {
        let mut query = 0;
        let mut transform = 0;
        let mut stream = 0;

        for beam in &self.beams {
            match beam.op_class {
                0 => query += 1,
                1 => transform += 1,
                2 => stream += 1,
                _ => {}
            }
        }

        (query, transform, stream)
    }

    /// Count beams by colour/priority
    ///
    /// Returns: (cold, warm, hot, critical, absolute)
    pub fn count_by_colour(&self) -> (usize, usize, usize, usize, usize) {
        let mut cold = 0;
        let mut warm = 0;
        let mut hot = 0;
        let mut critical = 0;
        let mut absolute = 0;

        for beam in &self.beams {
            match beam.colour {
                'b' => cold += 1,
                'g' => warm += 1,
                'y' => hot += 1,
                'r' => critical += 1,
                'w' => absolute += 1,
                _ => {}
            }
        }

        (cold, warm, hot, critical, absolute)
    }

    /// Filter beams by minimum priority weight
    pub fn filter_by_min_priority(&self, min_weight: f32) -> MoifeuBeamBatch {
        MoifeuBeamBatch {
            beams: self
                .beams
                .iter()
                .filter(|b| b.priority_weight() >= min_weight)
                .cloned()
                .collect(),
        }
    }

    /// Get all urgent beams (priority >= 0.50, i.e., hot and above)
    pub fn urgent_beams(&self) -> MoifeuBeamBatch {
        self.filter_by_min_priority(0.50)
    }

    /// Get all high priority beams (critical or absolute)
    pub fn high_priority_beams(&self) -> MoifeuBeamBatch {
        MoifeuBeamBatch {
            beams: self
                .beams
                .iter()
                .filter(|b| b.is_high_priority())
                .cloned()
                .collect(),
        }
    }

    /// Get all query beams
    pub fn query_beams(&self) -> MoifeuBeamBatch {
        MoifeuBeamBatch {
            beams: self
                .beams
                .iter()
                .filter(|b| b.is_query())
                .cloned()
                .collect(),
        }
    }

    /// Get all transform beams
    pub fn transform_beams(&self) -> MoifeuBeamBatch {
        MoifeuBeamBatch {
            beams: self
                .beams
                .iter()
                .filter(|b| b.is_transform())
                .cloned()
                .collect(),
        }
    }

    /// Get all stream beams
    pub fn stream_beams(&self) -> MoifeuBeamBatch {
        MoifeuBeamBatch {
            beams: self
                .beams
                .iter()
                .filter(|b| b.is_stream())
                .cloned()
                .collect(),
        }
    }

    /// Calculate estimated compression ratio
    ///
    /// Assumes average natural language length for comparison.
    /// Returns ratio of token savings (0.0 to 1.0).
    pub fn estimated_compression_ratio(&self, avg_natural_length: usize) -> f32 {
        if avg_natural_length == 0 {
            return 0.0;
        }

        let total_wire_length: usize = self.beams.iter().map(|b| b.raw.len()).sum();

        let total_natural_length = self.beams.len() * avg_natural_length;

        if total_natural_length == 0 {
            0.0
        } else {
            1.0 - (total_wire_length as f32 / total_natural_length as f32)
        }
    }

    /// Get the beam with highest priority
    pub fn highest_priority(&self) -> Option<&MoifeuBeam> {
        self.beams.iter().max_by(|a, b| {
            a.priority_weight()
                .partial_cmp(&b.priority_weight())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Convert all beams to SpacetimeDB push arguments
    pub fn to_spacetime_args(&self) -> Vec<(u8, f32, i8, String)> {
        self.beams
            .iter()
            .map(|b| b.to_spacetime_push_args())
            .collect()
    }
}

impl IntoIterator for MoifeuBeamBatch {
    type Item = MoifeuBeam;
    type IntoIter = std::vec::IntoIter<MoifeuBeam>;

    fn into_iter(self) -> Self::IntoIter {
        self.beams.into_iter()
    }
}

impl<'a> IntoIterator for &'a MoifeuBeamBatch {
    type Item = &'a MoifeuBeam;
    type IntoIter = std::slice::Iter<'a, MoifeuBeam>;

    fn into_iter(self) -> Self::IntoIter {
        self.beams.iter()
    }
}

impl FromIterator<MoifeuBeam> for MoifeuBeamBatch {
    fn from_iter<T: IntoIterator<Item = MoifeuBeam>>(iter: T) -> Self {
        MoifeuBeamBatch {
            beams: iter.into_iter().collect(),
        }
    }
}

// ============================================================================
// CACHING
// ============================================================================

lazy_static! {
    /// Global cache for parsed beams
    ///
    /// Stores beam strings and their parsed representations to avoid
    /// redundant parsing of frequently used beams.
    static ref BEAM_CACHE: RwLock<HashMap<String, MoifeuBeam>> =
        RwLock::new(HashMap::new());
}

impl MoifeuBeam {
    /// Parse a beam from cache or parse it fresh
    ///
    /// If the beam has been parsed before, returns the cached version.
    /// Otherwise, parses it and caches the result.
    ///
    /// # Example
    /// ```
    /// let beam1 = MoifeuBeam::from_cache_or_parse("112g435|tr hi>en")?;
    /// let beam2 = MoifeuBeam::from_cache_or_parse("112g435|tr hi>en")?; // Cache hit
    /// assert!(std::ptr::eq(&beam1, &beam2)); // Same instance (if cloned)
    /// ```
    pub fn from_cache_or_parse(raw: &str) -> ParseResult<Self> {
        {
            let cache = BEAM_CACHE
                .read()
                .map_err(|_| MoifeuError::ParseError("cache lock poisoned".into()))?;
            if let Some(beam) = cache.get(raw) {
                return Ok(beam.clone());
            }
        }

        let beam = parse_beam(raw)?;
        {
            let mut cache = BEAM_CACHE
                .write()
                .map_err(|_| MoifeuError::ParseError("cache lock poisoned".into()))?;
            cache.insert(raw.to_string(), beam.clone());
        }
        Ok(beam)
    }

    /// Clear the beam cache
    ///
    /// Useful for freeing memory or when beam definitions change.
    pub fn clear_cache() -> ParseResult<()> {
        let mut cache = BEAM_CACHE
            .write()
            .map_err(|_| MoifeuError::ParseError("cache lock poisoned".into()))?;
        cache.clear();
        Ok(())
    }

    /// Get cache size
    pub fn cache_size() -> ParseResult<usize> {
        let cache = BEAM_CACHE
            .read()
            .map_err(|_| MoifeuError::ParseError("cache lock poisoned".into()))?;
        Ok(cache.len())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_canonical() {
        let b = parse_beam("112g435|tr hi>en sm +json").unwrap();
        assert_eq!(b.op_class, 1);
        assert_eq!(b.colour, 'g');
        assert_eq!((b.depth, b.speed, b.certainty), (4, 3, 5));
    }
    #[test]
    fn parse_with_context() {
        let b = parse_beam("200r999|@spacetimedb ?fog ! #no_prose").unwrap();
        assert_eq!(b.context.as_deref(), Some("spacetimedb"));
    }
    #[test]
    fn rejects_bad_kappa() {
        assert!(parse_beam("112x435|tr hi>en").is_err());
    }
    #[test]
    fn rejects_bad_ternary() {
        assert!(parse_beam("392g435|tr hi>en").is_err());
    }
    #[test]
    fn round_trip() {
        let raw = "112g435|tr hi>en +json";
        let b = parse_beam(raw).unwrap();
        let out = emit_beam(&b);
        assert_eq!(parse_beam(&out).unwrap(), b);
    }
    #[test]
    fn builder_works() {
        let b = MoifeuBeamBuilder::new()
            .header(1, 1, 2)
            .kappa('g')
            .operation("tr")
            .input("test")
            .build()
            .unwrap();
        assert_eq!(b.op_class, 1);
    }
    #[test]
    fn macro_simple() {
        let b = moifeu!('g', "tr", "hello");
        assert_eq!(b.colour, 'g');
    }
    #[test]
    fn macro_query() {
        let b = moifeu!(query: "what");
        assert_eq!(b.op_class, 0);
    }
    #[test]
    fn priority_weight() {
        assert_eq!(moifeu!('b', "an", "x").priority_weight(), 0.0);
        assert_eq!(moifeu!('g', "an", "x").priority_weight(), 0.25);
    }
    #[test]
    fn spacetime_args() {
        let b = moifeu!('y', "tr", "test");
        let (seat, density, stance, _) = b.to_spacetime_push_args();
        assert_eq!(seat, 0);
        assert!((density - 4.5).abs() < 0.01);
        assert_eq!(stance, 1);
    }
    #[test]
    fn backward_compat() {
        let b = parse_beam_compat("112g435|tr hi>en").unwrap();
        assert_eq!(b.op_class, 1);
    }

    // --- Batch Tests ---
    #[test]
    fn batch_basic() {
        let mut batch = MoifeuBeamBatch::new();
        batch.add(moifeu!('g', "an", "first"));
        batch.add(moifeu!('r', "an", "second"));
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn batch_priority_sort() {
        let mut batch = MoifeuBeamBatch::new();
        batch.add(moifeu!('b', "an", "low"));
        batch.add(moifeu!('r', "an", "high"));
        batch.add(moifeu!('g', "an", "medium"));
        batch.sort_by_priority();
        let beams: Vec<_> = batch.into_iter().collect();
        assert_eq!(beams[0].colour, 'r');
        assert_eq!(beams[1].colour, 'g');
        assert_eq!(beams[2].colour, 'b');
    }

    #[test]
    fn batch_count_by_class() {
        let mut batch = MoifeuBeamBatch::new();
        batch.add(moifeu!(query: "q1"));
        batch.add(moifeu!(query: "q2"));
        batch.add(moifeu!(transform: "tr", "t1"));
        batch.add(moifeu!(stream: "sm", "s1"));
        let (q, t, s) = batch.count_by_class();
        assert_eq!(q, 2);
        assert_eq!(t, 1);
        assert_eq!(s, 1);
    }

    #[test]
    fn batch_filter_urgent() {
        let mut batch = MoifeuBeamBatch::new();
        batch.add(moifeu!('g', "an", "normal"));
        batch.add(moifeu!('r', "an", "urgent"));
        batch.add(moifeu!('b', "an", "low"));
        let urgent = batch.urgent_beams();
        assert_eq!(urgent.len(), 1);
        assert_eq!(urgent.beams()[0].colour, 'r');
    }

    #[test]
    fn batch_from_iter() {
        let beams = vec![moifeu!('g', "an", "a"), moifeu!('g', "an", "b")];
        let batch: MoifeuBeamBatch = beams.into_iter().collect();
        assert_eq!(batch.len(), 2);
    }

    // --- Cache Tests ---
    #[test]
    fn cache_basic() {
        MoifeuBeam::clear_cache().unwrap();
        let beam1 = MoifeuBeam::from_cache_or_parse("112g435|tr hi>en").unwrap();
        let beam2 = MoifeuBeam::from_cache_or_parse("112g435|tr hi>en").unwrap();
        assert_eq!(beam1, beam2);
        let size = MoifeuBeam::cache_size().unwrap();
        assert_eq!(size, 1, "Expected cache size 1, got {}", size);
    }

    #[test]
    fn cache_clear() {
        MoifeuBeam::clear_cache().unwrap();
        let _ = MoifeuBeam::from_cache_or_parse("112g435|tr hi>en").unwrap();
        assert_eq!(MoifeuBeam::cache_size().unwrap(), 1);
        MoifeuBeam::clear_cache().unwrap();
        assert_eq!(MoifeuBeam::cache_size().unwrap(), 0);
    }

    #[test]
    fn cache_multiple() {
        MoifeuBeam::clear_cache().unwrap();
        let _ = MoifeuBeam::from_cache_or_parse("112g435|tr hi>en").unwrap();
        let _ = MoifeuBeam::from_cache_or_parse("200r999|an sm +json").unwrap();
        let size = MoifeuBeam::cache_size().unwrap();
        assert_eq!(size, 2, "Expected cache size 2, got {}", size);
    }

    // --- Zed Gradient Map Tests ---
    #[test]
    fn gradient_cell_from_beam() {
        let beam = moifeu!('g', "an", "test");
        let cell = GradientCell::from_beam(&beam, "TestSeat".to_string());
        assert_eq!(cell.name, "TestSeat");
        assert_eq!(cell.density, 4.5); // depth 5 -> density = 5/10*9 = 4.5
    }

    #[test]
    fn gradient_map_from_batch() {
        let mut batch = MoifeuBeamBatch::new();
        batch.add(moifeu!('g', "an", "a"));
        batch.add(moifeu!('r', "an", "b"));
        let map = GradientMap::from_batch(&batch);
        assert_eq!(map.cells.len(), 2);
    }

    #[test]
    fn yield_target_from_beam() {
        let beam = moifeu!(urgent: "gen", "target_123"); // "gen" is a valid op
        let target = YieldTarget::from_beam(&beam, 123, 100.0);
        assert_eq!(target.payload_hash, "target_123");
        assert_eq!(target.estimated_value_usdc, 100.0);
    }

    #[test]
    fn zed_wrapper_parse() {
        let beam = moifeu!('g', "tr", "hello");
        let zed_beam = ZedMoifeuBeam::from_moifeu(beam);
        assert_eq!(zed_beam.beam.op_class, 1);
        assert_eq!(zed_beam.priority_score(), 0.5); // priority_weight 0.25 * 2 = 0.5
    }

    #[test]
    fn zed_wrapper_gradient_integration() {
        let beam = moifeu!('y', "an", "query");
        let zed_beam = ZedMoifeuBeam::from_moifeu(beam);
        let cell = zed_beam.to_gradient_cell("Test".to_string());
        assert_eq!(cell.hue, 60); // yellow hue
    }
}

// ============================================================================
// ZED INTEGRATION
// ============================================================================

/// Represents a cell in the color gradient map from SpacetimeDB's gradient_map procedure.
/// This is the data communication layer between Zed agents and the fog field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientCell {
    pub seat_id: u8,
    pub name: String,
    pub density: f32,
    pub stance: i8,
    pub angle: u16,
    pub hue: u16,
    pub sat: u8,
    pub whisper: String,
}

impl GradientCell {
    /// Create a GradientCell from a MoifeuBeam
    pub fn from_beam(beam: &MoifeuBeam, name: String) -> Self {
        let density = beam.depth as f32 / 10.0 * 9.0;
        let stance = match beam.colour {
            'b' => -1,
            'g' => 0,
            'y' | 'r' | 'w' => 1,
            _ => 0,
        };
        let hue = match beam.colour {
            'b' => 210,
            'g' => 140,
            'y' => 60,
            'r' => 20,
            'w' => 0,
            _ => 140,
        };
        let sat = (beam.depth as f32 / 9.0 * 100.0) as u8;

        GradientCell {
            seat_id: beam
                .context
                .as_ref()
                .and_then(|c| c.parse::<u8>().ok())
                .unwrap_or(0),
            name,
            density,
            stance,
            angle: 0,
            hue,
            sat,
            whisper: beam.input.clone().unwrap_or_default(),
        }
    }

    /// Convert to a MoifeuBeam
    pub fn to_beam(&self) -> MoifeuBeam {
        let depth = ((self.sat as f32 / 100.0) * 9.0).round() as u8;
        let colour = match self.hue {
            181..=260 => 'b',
            81..=180 => 'g',
            41..=80 => 'y',
            0..=40 | 320..=360 => 'r',
            _ => 'g',
        };

        MoifeuBeamBuilder::new()
            .kappa(colour)
            .analogue(depth, 5, 5)
            .context(self.name.clone())
            .input(self.whisper.clone())
            .build()
            .unwrap_or_else(|_| MoifeuBeam::default())
    }
}

/// A collection of GradientCells representing the full fog field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GradientMap {
    pub cells: Vec<GradientCell>,
}

impl GradientMap {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_capacity(cap: usize) -> Self {
        GradientMap {
            cells: Vec::with_capacity(cap),
        }
    }
    pub fn add(&mut self, cell: GradientCell) {
        self.cells.push(cell);
    }
    pub fn get(&self, seat_id: u8) -> Option<&GradientCell> {
        self.cells.iter().find(|c| c.seat_id == seat_id)
    }
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn from_batch(batch: &MoifeuBeamBatch) -> Self {
        GradientMap {
            cells: batch
                .beams
                .iter()
                .enumerate()
                .map(|(i, beam)| {
                    let name = format!("seat_{}", i);
                    GradientCell::from_beam(beam, name)
                })
                .collect(),
        }
    }
    pub fn avg_density(&self) -> f32 {
        if self.cells.is_empty() {
            0.0
        } else {
            self.cells.iter().map(|c| c.density).sum::<f32>() / self.cells.len() as f32
        }
    }
    pub fn to_batch(&self) -> MoifeuBeamBatch {
        MoifeuBeamBatch::from_iter(self.cells.iter().map(|c| c.to_beam()))
    }
}

/// Yield target for token bucket management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldTarget {
    pub target_id: u64,
    pub source_protocol: String,
    pub payload_hash: String,
    pub estimated_value_usdc: f32,
    pub execution_complexity: u8,
    pub status: i8,
    pub note: String,
    /// Epoch microseconds at finalization (for compaction). Only valid for dropped/executed targets.
    #[serde(default)]
    pub created_at: u64,
}

impl YieldTarget {
    pub fn from_beam(beam: &MoifeuBeam, target_id: u64, value: f32) -> Self {
        let complexity = match beam.op_level {
            0 => 1,
            1 => 3,
            2 => 5,
            _ => 3,
        };
        YieldTarget {
            target_id,
            source_protocol: beam.op.clone().unwrap_or_else(|| "unknown".to_string()),
            payload_hash: beam.input.clone().unwrap_or_else(|| beam.raw.clone()),
            estimated_value_usdc: value,
            execution_complexity: complexity,
            status: 0,
            note: beam.context.clone().unwrap_or_default(),
            created_at: 0,
        }
    }
    pub fn is_queued(&self) -> bool {
        self.status == 0
    }
    pub fn is_locked(&self) -> bool {
        self.status == 1
    }
    pub fn is_executed(&self) -> bool {
        self.status == 2
    }
    pub fn is_dropped(&self) -> bool {
        self.status == -1
    }
    pub fn to_beam(&self) -> MoifeuBeam {
        let op_class = match self.execution_complexity {
            1 => 0,
            2..=3 => 1,
            4..=5 => 2,
            _ => 1,
        };
        MoifeuBeamBuilder::new()
            .header(op_class, self.execution_complexity, 2)
            .kappa('g')
            .operation(&self.source_protocol)
            .input(&self.payload_hash)
            .context(&self.note)
            .build()
            .unwrap_or_else(|_| MoifeuBeam::default())
    }
}

/// Yield ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldLedger {
    pub tx_id: u64,
    pub target_id: u64,
    pub value_captured: f32,
    pub note: String,
    /// Epoch microseconds at execution time (for compaction).
    #[serde(default)]
    pub executed_at: u64,
}

/// Token bucket manager.
#[derive(Debug, Clone, Default)]
pub struct TokenBucket {
    pub queued: Vec<YieldTarget>,
    pub locked: Vec<YieldTarget>,
    pub executed: Vec<YieldLedger>,
    pub dropped: Vec<YieldTarget>,
}

impl TokenBucket {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn enqueue(&mut self, target: YieldTarget) {
        self.queued.push(target);
    }
    pub fn lock(&mut self, target_id: u64) -> Option<YieldTarget> {
        let idx = self.queued.iter().position(|t| t.target_id == target_id)?;
        let mut target = self.queued.remove(idx);
        target.status = 1;
        target.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        self.locked.push(target.clone());
        Some(target)
    }
    pub fn execute(
        &mut self,
        target_id: u64,
        tx_id: u64,
        value: f32,
        now_us: Option<u64>,
    ) -> Option<YieldLedger> {
        let idx = self.locked.iter().position(|t| t.target_id == target_id)?;
        let target = self.locked.remove(idx);
        let ledger = YieldLedger {
            tx_id,
            target_id,
            value_captured: value,
            note: format!("{} executed", target.source_protocol),
            executed_at: now_us.unwrap_or(0),
        };
        self.executed.push(ledger.clone());
        Some(ledger)
    }
    pub fn drop(&mut self, target_id: u64, now_us: Option<u64>) -> Option<YieldTarget> {
        for vec in [&mut self.queued, &mut self.locked] {
            if let Some(idx) = vec.iter().position(|t| t.target_id == target_id) {
                let mut target = vec.remove(idx);
                target.status = -1;
                target.created_at = now_us.unwrap_or(0);
                self.dropped.push(target.clone());
                return Some(target);
            }
        }
        None
    }
    pub fn queued_count(&self) -> usize {
        self.queued.len()
    }
    pub fn locked_count(&self) -> usize {
        self.locked.len()
    }
    pub fn executed_count(&self) -> usize {
        self.executed.len()
    }
    pub fn dropped_count(&self) -> usize {
        self.dropped.len()
    }
    pub fn total_value(&self) -> f32 {
        self.executed.iter().map(|l| l.value_captured).sum()
    }

    /// Compact old entries to prevent unbounded growth.
    pub fn compact(&mut self, max_age_us: u64, now_us: u64, max_entries: usize) {
        let age_cutoff = now_us.saturating_sub(max_age_us);

        if self.executed.len() > max_entries {
            let min_keep = 50.min(self.executed.len());
            let mut sorted_indices: Vec<(usize, f32)> = self
                .executed
                .iter()
                .enumerate()
                .map(|(i, l)| (i, l.value_captured))
                .collect();
            sorted_indices
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_k: std::collections::HashSet<usize> = sorted_indices
                .iter()
                .take(min_keep)
                .map(|&(i, _)| i)
                .collect();
            let executed_len = self.executed.len();

            self.executed.retain(|l| {
                l.executed_at == 0
                    || l.executed_at > age_cutoff
                    || top_k.contains(&(l.tx_id as usize % max(executed_len, 1)))
            });
        } else if !self.executed.is_empty() {
            self.executed
                .retain(|l| l.executed_at == 0 || l.executed_at > age_cutoff);
        }

        if self.dropped.len() > max_entries / 2 {
            self.dropped
                .retain(|t| t.created_at == 0 || t.created_at > age_cutoff);
        } else if !self.dropped.is_empty() {
            self.dropped
                .retain(|t| t.created_at == 0 || t.created_at > age_cutoff);
        }
    }

    /// Convenience: compact entries older than N hours.
    pub fn compact_by_hours(&mut self, max_age_hours: u64, max_entries: usize) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        self.compact(max_age_hours * 3600 * 1_000_000, now, max_entries);
    }

    /// Recycle locked targets that have been stuck for too long.
    pub fn stale_locks(&mut self, threshold_us: u64) -> usize {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        let cutoff = now.saturating_sub(threshold_us);

        let mut recycled = 0;
        self.locked.retain(|t| {
            if t.created_at == 0 || t.created_at > cutoff {
                true
            } else {
                recycled += 1;
                false
            }
        });
        recycled
    }

    /// Auto-compact with sensible defaults for Zed / Spacetime flood volumes.
    /// Keeps last 24h of data, max 500 entries per bucket, recycles locks > 2h.
    pub fn auto_compact(&mut self) -> CompactResult {
        let recycled = self.stale_locks(7200 * 1_000_000);
        let max_entries = 500;
        let had_executed = self.executed.len();
        let had_dropped = self.dropped.len();

        self.compact_by_hours(24, max_entries);

        CompactResult {
            recycled_stale_locks: recycled,
            compacted_executed: had_executed - self.executed.len(),
            compacted_dropped: had_dropped - self.dropped.len(),
            stale_locked_remaining: recycled,
        }
    }

    pub fn summary(&self) -> YieldSummary {
        YieldSummary {
            queued: self.queued_count() as u64,
            locked: self.locked_count() as u64,
            executed: self.executed_count() as u64,
            dropped: self.dropped_count() as u64,
            total_captured_usdc: self.total_value(),
            captured_last_7d_usdc: 0.0,
        }
    }
}

/// Yield summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldSummary {
    pub queued: u64,
    pub locked: u64,
    pub executed: u64,
    pub dropped: u64,
    pub total_captured_usdc: f32,
    pub captured_last_7d_usdc: f32,
}

/// Compact result tracking.
#[derive(Debug, Clone, Default)]
pub struct CompactResult {
    pub recycled_stale_locks: usize,
    pub compacted_executed: usize,
    pub compacted_dropped: usize,
    pub stale_locked_remaining: usize,
}

/// Zed-specific Moifeu beam wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZedMoifeuBeam {
    pub beam: MoifeuBeam,
    pub zed_priority: f32,
    pub gradient_cell: Option<GradientCell>,
    pub yield_target: Option<YieldTarget>,
}

impl ZedMoifeuBeam {
    pub fn from_moifeu(beam: MoifeuBeam) -> Self {
        let priority = beam.priority_weight() * 2.0;
        ZedMoifeuBeam {
            beam,
            zed_priority: priority,
            gradient_cell: None,
            yield_target: None,
        }
    }
    pub fn from_gradient_cell(cell: GradientCell) -> Self {
        let beam = cell.to_beam();
        ZedMoifeuBeam {
            beam,
            zed_priority: (cell.density / 9.0) * cell.sat as f32 / 100.0,
            gradient_cell: Some(cell),
            yield_target: None,
        }
    }
    pub fn from_yield_target(target: YieldTarget) -> Self {
        let beam = target.to_beam();
        let zed_priority = match target.execution_complexity {
            1 => 0.25,
            2..=3 => 0.50,
            4..=5 => 0.75,
            _ => 0.50,
        };
        ZedMoifeuBeam {
            beam,
            zed_priority,
            gradient_cell: None,
            yield_target: Some(target),
        }
    }
    pub fn priority_score(&self) -> f32 {
        self.zed_priority
    }
    pub fn is_high_priority(&self) -> bool {
        self.zed_priority >= 0.75
    }
    pub fn into_beam(self) -> MoifeuBeam {
        self.beam
    }
    pub fn to_gradient_cell(&self, name: String) -> GradientCell {
        GradientCell::from_beam(&self.beam, name)
    }
    pub fn to_yield_target(&self, target_id: u64, value: f32) -> YieldTarget {
        YieldTarget::from_beam(&self.beam, target_id, value)
    }
    pub fn should_cache(&self) -> bool {
        self.beam.certainty >= 7 && self.beam.depth >= 5
    }
}

impl From<MoifeuBeam> for ZedMoifeuBeam {
    fn from(beam: MoifeuBeam) -> Self {
        Self::from_moifeu(beam)
    }
}
impl From<GradientCell> for ZedMoifeuBeam {
    fn from(cell: GradientCell) -> Self {
        Self::from_gradient_cell(cell)
    }
}
impl From<YieldTarget> for ZedMoifeuBeam {
    fn from(target: YieldTarget) -> Self {
        Self::from_yield_target(target)
    }
}

/// Zed batch processor.
#[derive(Debug, Clone, Default)]
pub struct ZedMoifeuBatch {
    pub beams: Vec<ZedMoifeuBeam>,
    pub gradient_map: Option<GradientMap>,
}

impl ZedMoifeuBatch {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, beam: ZedMoifeuBeam) {
        self.beams.push(beam);
    }
    pub fn add_moifeu(&mut self, beam: MoifeuBeam) {
        self.beams.push(ZedMoifeuBeam::from_moifeu(beam));
    }
    pub fn add_gradient_cell(&mut self, cell: GradientCell) {
        self.beams.push(ZedMoifeuBeam::from_gradient_cell(cell));
    }
    pub fn set_gradient_map(&mut self, map: GradientMap) {
        self.gradient_map = Some(map);
    }
    pub fn sorted_by_priority(&self) -> Vec<&ZedMoifeuBeam> {
        let mut b: Vec<&ZedMoifeuBeam> = self.beams.iter().collect();
        b.sort_by(|a, b| {
            b.priority_score()
                .partial_cmp(&a.priority_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        b
    }
    pub fn high_priority(&self) -> Vec<&ZedMoifeuBeam> {
        self.beams.iter().filter(|b| b.is_high_priority()).collect()
    }
    pub fn total_priority(&self) -> f32 {
        self.beams.iter().map(|b| b.priority_score()).sum()
    }
    pub fn to_moifeu_batch(&self) -> MoifeuBeamBatch {
        MoifeuBeamBatch::from_iter(self.beams.iter().map(|b| b.beam.clone()))
    }
    pub fn to_gradient_map(&self) -> GradientMap {
        if let Some(ref map) = self.gradient_map {
            return map.clone();
        }
        GradientMap::from_batch(&self.to_moifeu_batch())
    }
}

impl FromIterator<ZedMoifeuBeam> for ZedMoifeuBatch {
    fn from_iter<T: IntoIterator<Item = ZedMoifeuBeam>>(iter: T) -> Self {
        ZedMoifeuBatch {
            beams: iter.into_iter().collect(),
            gradient_map: None,
        }
    }
}

// Re-export gradient codec functions for convenience
pub use moifeu_gradient::{
    beams_to_gradient_cells, beams_to_wire_format, decode_beams_to_bytes,
    decode_beams_to_text, decode_from_lenia_field, encode_bytes_to_beams,
    encode_text_to_beams, encode_to_lenia_field, gradient_cells_to_beams,
    wire_format_to_beams, MoifeuGradientError, BYTES_PER_BEAM, MAX_PAYLOAD_BYTES,
    CHECKSUM_MOD, SENTINEL_START_OP, SENTINEL_END_OP,
};
