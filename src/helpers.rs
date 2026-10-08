pub struct Visual {
    pub v_x: usize,
    pub v_y: usize,
    pub on: bool,
}

impl Visual {
    pub fn new() -> Self {
        Visual {
            v_x: 0,
            v_y: 0,
            on: false,
        }
    }
}

fn wcag_contrast(l1: f64, l2: f64) -> f64 {
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// resolve a hue across every opaline theme. only the catppuccin
/// family defines raw names like "blue" or "mauve", so each hue falls
/// back through shared semantic tokens. never returns FALLBACK.
pub fn hue(theme: &opaline::Theme, hue: &str) -> opaline::OpalineColor {
    if let Some(c) = theme.try_color(hue) {
        return c;
    }
    // Map missing theme keys strictly to the theme's native semantic tokens
    let mapped = match hue {
        "blue" | "sapphire" | "sky" | "teal" => "accent.secondary",
        "green" => "success",
        "mauve" | "pink" | "lavender" => "accent.primary",
        "peach" | "yellow" => "warning",
        "red" => "error",
        _ => "accent.primary",
    };
    if let Some(c) = theme.try_color(mapped) {
        return c;
    }
    theme
        .try_color("accent.primary")
        .or_else(|| theme.try_color("text.primary"))
        .unwrap_or(opaline::OpalineColor {
            r: 203,
            g: 166,
            b: 247,
        })
}

/// resolve a structural token with a safe fallback chain.
pub fn tok(theme: &opaline::Theme, key: &str) -> opaline::OpalineColor {
    let fallbacks: &[&str] = match key {
        "border.unfocused" => &["text.dim"],
        "accent.deep" => &["accent.primary"],
        _ => &[],
    };
    theme
        .try_color(key)
        .or_else(|| fallbacks.iter().find_map(|k| theme.try_color(k)))
        .unwrap_or_else(|| theme.color("text.primary"))
}

/// blend a color toward the theme background. ratatui and terminals
/// have no alpha channel, so translucent colors are pre-multiplied
/// here instead of being silently ignored.
pub fn blend(
    fg: opaline::OpalineColor,
    bg: opaline::OpalineColor,
    alpha: f32,
) -> opaline::OpalineColor {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |f: u8, b: u8| {
        ((f as f32 * a) + (b as f32 * (1.0 - a)))
            .round()
            .clamp(0.0, 255.0) as u8
    };
    opaline::OpalineColor {
        r: mix(fg.r, bg.r),
        g: mix(fg.g, bg.g),
        b: mix(fg.b, bg.b),
    }
}

/// translucent color over the theme background, ready for ratatui.
pub fn fade(theme: &opaline::Theme, c: opaline::OpalineColor, alpha: f32) -> ratatui::style::Color {
    blend(c, theme.color("bg.base"), alpha).into()
}

/// same, for a color already converted to ratatui.
pub fn fade_rgb(
    theme: &opaline::Theme,
    c: ratatui::style::Color,
    alpha: f32,
) -> ratatui::style::Color {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => blend(
            opaline::OpalineColor { r, g, b },
            theme.color("bg.base"),
            alpha,
        )
        .into(),
        other => other,
    }
}

pub fn fg_color(bg: opaline::OpalineColor) -> ratatui::style::Color {
    let to_linear = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * to_linear(bg.r) + 0.7152 * to_linear(bg.g) + 0.0722 * to_linear(bg.b);

    let contrast_black = wcag_contrast(luminance, 0.0);
    let contrast_white = wcag_contrast(luminance, 1.0);

    if contrast_black >= contrast_white {
        ratatui::style::Color::Black
    } else {
        ratatui::style::Color::White
    }
}

/// Extensions we accept that syntect has no grammar for, mapped onto the
/// closest bundled one. Covers the long tail of modern languages without
/// shipping third party grammars.
const EXT_ALIASES: &[(&str, &str)] = &[
    // typed and bundled javascript flavours
    ("ts", "js"),
    ("tsx", "js"),
    ("jsx", "js"),
    ("mts", "js"),
    ("cts", "js"),
    ("mjs", "js"),
    ("cjs", "js"),
    ("svelte", "html"),
    ("vue", "html"),
    ("astro", "html"),
    ("marko", "html"),
    ("hbs", "html"),
    ("handlebars", "html"),
    ("mustache", "html"),
    ("jinja", "html"),
    ("jinja2", "html"),
    ("j2", "html"),
    ("twig", "html"),
    ("pug", "html"),
    ("jade", "html"),
    ("haml", "html"),
    ("erb", "html"),
    // jvm family
    ("kt", "java"),
    ("kts", "java"),
    ("dart", "java"),
    ("groovy2", "groovy"),
    ("scala3", "scala"),
    ("sc", "scala"),
    // c family
    ("swift", "cpp"),
    ("cxx", "cpp"),
    ("c++", "cpp"),
    ("hxx", "cpp"),
    ("hh", "cpp"),
    ("hpp", "cpp"),
    ("h++", "cpp"),
    ("ipp", "cpp"),
    ("inl", "cpp"),
    ("mm", "mm"),
    ("zig", "c"),
    ("awk", "c"),
    ("gawk", "c"),
    ("sed", "c"),
    ("thrift", "c"),
    ("sol", "cpp"),
    ("proto", "cpp"),
    ("nix", "js"),
    // functional, mapped onto the nearest bundled grammar
    ("ex", "erl"),
    ("exs", "erl"),
    ("eex", "erl"),
    ("heex", "erl"),
    ("hrl", "erl"),
    ("cljs", "clj"),
    ("cljc", "clj"),
    ("edn", "clj"),
    ("nu", "clj"),
    ("elm", "hs"),
    ("purs", "hs"),
    ("rkt", "scm"),
    ("rktl", "scm"),
    ("guile", "scm"),
    ("sld", "scm"),
    ("ss", "scm"),
    ("fs", "ml"),
    ("fsx", "ml"),
    ("fsi", "ml"),
    ("ppx", "ml"),
    ("jl", "matlab"),
    // scripting
    ("ps1", "sh"),
    ("psm1", "sh"),
    ("psd1", "sh"),
    ("cr", "rb"),
    ("nim", "py"),
    ("rq", "py"),
    ("sparql", "py"),
    ("spec", "py"),
    // data and config, both are flat key/value formats
    ("toml", "yaml"),
    ("ini", "properties"),
    ("cfg", "properties"),
    ("conf", "properties"),
    ("env", "properties"),
    ("desktop", "properties"),
    ("service", "properties"),
    ("timer", "properties"),
    ("mount", "properties"),
    ("tf", "yaml"),
    ("tfvars", "yaml"),
    ("hcl", "yaml"),
    ("bicep", "json"),
    // stylesheets
    ("scss", "css"),
    ("sass", "css"),
    ("less", "css"),
    ("styl", "css"),
    // docs
    ("adoc", "rst"),
    ("asciidoc", "rst"),
    // infra
    ("cmake", "make"),
    ("mk", "make"),
    ("am", "make"),
    ("ac", "make"),
];

/// whole file names that carry a language with no usable extension
const NAME_ALIASES: &[(&str, &str)] = &[
    ("dockerfile", "sh"),
    ("containerfile", "sh"),
    ("cmakelists.txt", "make"),
    ("makefile.am", "make"),
    ("makefile.in", "make"),
    ("gnumakefile", "make"),
    ("justfile", "make"),
    ("kbuild", "make"),
    ("rakefile", "rb"),
    ("gemfile", "rb"),
    ("vagrantfile", "rb"),
    ("brewfile", "rb"),
    ("guardfile", "rb"),
    ("capfile", "rb"),
    ("berksfile", "rb"),
    ("thorfile", "rb"),
    ("podfile", "rb"),
    ("fastfile", "rb"),
    ("appfile", "rb"),
    ("deliverfile", "rb"),
    ("matchfile", "rb"),
    ("snapfile", "rb"),
    ("puppetfile", "rb"),
    ("buildfile", "rb"),
    ("gemfile.lock", "rb"),
    ("cargo.lock", "yaml"),
    ("cargo.toml", "yaml"),
    ("pyproject.toml", "yaml"),
    ("poetry.lock", "yaml"),
    ("pdm.lock", "yaml"),
    ("pixi.lock", "yaml"),
    ("requirements.txt", "properties"),
    ("constraints.txt", "properties"),
    ("pipfile", "properties"),
    ("sconstruct", "py"),
    ("sconscript", "py"),
    ("wscript", "py"),
    ("snakefile", "py"),
    ("cmakecache.txt", "make"),
    ("jenkinsfile", "groovy"),
    ("procfile", "yaml"),
    ("sshd_config", "properties"),
    ("my.cnf", "properties"),
    ("resolv.conf", "properties"),
    ("fstab", "properties"),
    ("crontab", "properties"),
    (".gitignore", "txt"),
    (".dockerignore", "txt"),
    (".npmignore", "txt"),
    (".gitattributes", "txt"),
    (".gitmodules", "properties"),
    (".gitconfig", "properties"),
    (".editorconfig", "properties"),
    (".env", "properties"),
    (".env.local", "properties"),
    (".eslintrc", "json"),
    (".prettierrc", "json"),
    (".babelrc", "json"),
    ("tsconfig.json", "json"),
    ("jsconfig.json", "json"),
    ("composer.json", "json"),
    ("composer.lock", "json"),
    ("package.json", "json"),
    ("package-lock.json", "json"),
    ("deno.json", "json"),
    ("deno.jsonc", "json"),
    ("biome.json", "json"),
    (".bashrc", "sh"),
    (".bash_profile", "sh"),
    (".bash_aliases", "sh"),
    (".bash_logout", "sh"),
    (".bash_functions", "sh"),
    (".bash_variables", "sh"),
    (".profile", "sh"),
    (".zshrc", "sh"),
    (".zshenv", "sh"),
    ("license", "txt"),
    ("licence", "txt"),
    ("copying", "txt"),
    ("notice", "txt"),
    ("authors", "txt"),
    ("changelog", "txt"),
    ("readme", "txt"),
    ("todo", "txt"),
    ("caddyfile", "txt"),
    ("nginx.conf", "txt"),
    ("httpd.conf", "txt"),
    ("apache2.conf", "txt"),
];

/// display names for extensions that borrow another language's grammar
const ALIAS_LABELS: &[(&str, &str)] = &[
    ("ts", "typescript"),
    ("tsx", "tsx"),
    ("mts", "typescript"),
    ("cts", "typescript"),
    ("jsx", "jsx"),
    ("kt", "kotlin"),
    ("kts", "kotlin"),
    ("dart", "dart"),
    ("swift", "swift"),
    ("ex", "elixir"),
    ("exs", "elixir"),
    ("jl", "julia"),
    ("zig", "zig"),
    ("nim", "nim"),
    ("cr", "crystal"),
    ("vue", "vue"),
    ("svelte", "svelte"),
    ("astro", "astro"),
    ("toml", "toml"),
    ("ini", "ini"),
    ("cfg", "ini"),
    ("conf", "ini"),
    ("env", "ini"),
    ("scss", "scss"),
    ("sass", "sass"),
    ("less", "less"),
    ("ps1", "powershell"),
    ("psm1", "powershell"),
    ("proto", "protobuf"),
    ("tf", "terraform"),
    ("tfvars", "terraform"),
    ("hcl", "hcl"),
    ("graphql", "graphql"),
    ("erb", "erb"),
    ("hbs", "handlebars"),
    ("jinja", "jinja"),
    ("twig", "twig"),
    ("pug", "pug"),
    ("cmake", "cmake"),
    ("objc", "objective-c"),
];

/// display names for whole file names that carry their own language
const NAME_LABELS: &[(&str, &str)] = &[
    ("dockerfile", "dockerfile"),
    ("containerfile", "dockerfile"),
    ("cmakelists.txt", "cmake"),
    ("cmakecache.txt", "cmake"),
    ("makefile.am", "makefile"),
    ("makefile.in", "makefile"),
    ("gnumakefile", "makefile"),
    ("justfile", "makefile"),
    ("kbuild", "makefile"),
    ("cargo.lock", "toml"),
    ("cargo.toml", "toml"),
    ("pyproject.toml", "toml"),
    ("poetry.lock", "toml"),
    ("pdm.lock", "toml"),
    ("pixi.lock", "toml"),
    ("requirements.txt", "ini"),
    ("constraints.txt", "ini"),
    ("pipfile", "ini"),
    ("jenkinsfile", "groovy"),
    ("procfile", "yaml"),
    ("sshd_config", "ini"),
    ("my.cnf", "ini"),
    ("resolv.conf", "ini"),
    ("fstab", "ini"),
    ("crontab", "ini"),
    (".gitignore", "gitignore"),
    (".dockerignore", "dockerignore"),
    (".npmignore", "npmignore"),
    (".gitattributes", "gitattributes"),
    (".editorconfig", "editorconfig"),
    (".bashrc", "shell"),
    (".bash_profile", "shell"),
    (".bash_aliases", "shell"),
    (".bash_logout", "shell"),
    (".bash_functions", "shell"),
    (".bash_variables", "shell"),
    (".profile", "shell"),
    (".zshrc", "shell"),
    (".zshenv", "shell"),
];

/// shebang interpreters mapped onto a bundled extension
const SHEBANGS: &[(&str, &str)] = &[
    ("python", "py"),
    ("pypy", "py"),
    ("node", "js"),
    ("deno", "js"),
    ("bun", "js"),
    ("tsx", "js"),
    ("ts-node", "js"),
    ("bash", "sh"),
    ("zsh", "sh"),
    ("sh", "sh"),
    ("dash", "sh"),
    ("ksh", "sh"),
    ("fish", "sh"),
    ("ruby", "rb"),
    ("jruby", "rb"),
    ("perl", "pl"),
    ("php", "php"),
    ("lua", "lua"),
    ("luajit", "lua"),
    ("tclsh", "tcl"),
    ("wish", "tcl"),
    ("awk", "c"),
    ("gawk", "c"),
    ("sed", "c"),
    ("escript", "erl"),
    ("elixir", "erl"),
    ("iex", "erl"),
    ("julia", "matlab"),
    ("rscript", "R"),
    ("ocaml", "ml"),
    ("ocamlrun", "ml"),
    ("utop", "ml"),
    ("ghc", "hs"),
    ("runhaskell", "hs"),
    ("stack", "hs"),
    ("racket", "scm"),
    ("guile", "scm"),
    ("chicken", "scm"),
    ("csi", "scm"),
    ("pwsh", "sh"),
    ("powershell", "sh"),
    ("dart", "java"),
    ("groovy", "groovy"),
    ("scala", "scala"),
    ("sbt", "scala"),
    ("kotlinc", "java"),
    ("crystal", "rb"),
    ("nim", "py"),
    ("zig", "c"),
    ("iverilog", "txt"),
    ("verilator", "txt"),
];

/// weighted content signatures. each entry scores a bundled extension;
/// the highest score above the noise floor wins, so one stray word
/// cannot flip detection the way a first match did.
/// signatures carry a display label, since several languages share a
/// bundled grammar and should still name themselves in the status bar
type Signature<'a> = (&'a str, &'a str, &'a [(&'a str, u32)]);

const SIGNATURES: &[Signature<'_>] = &[
    (
        "rs",
        "rust",
        &[
            ("fn main", 4),
            ("let mut", 3),
            ("impl ", 2),
            ("#[derive", 4),
            ("use std::", 3),
            ("pub fn", 3),
            ("match ", 1),
            ("-> ", 1),
        ],
    ),
    (
        "go",
        "go",
        &[
            ("package main", 5),
            ("func main", 4),
            (":=", 2),
            ("import (", 2),
            ("fmt.", 2),
            ("err != nil", 3),
        ],
    ),
    (
        "py",
        "python",
        &[
            ("def ", 3),
            ("import ", 1),
            ("from ", 1),
            ("self.", 2),
            ("print(", 2),
            ("elif", 2),
            ("if __name__", 5),
            ("lambda ", 2),
            ("return ", 1),
            ("none", 1),
        ],
    ),
    (
        "rb",
        "ruby",
        &[
            ("def ", 2),
            ("puts ", 2),
            ("require '", 3),
            ("attr_accessor", 4),
            ("do |", 2),
            ("@", 1),
            ("end\n", 1),
        ],
    ),
    (
        "php",
        "php",
        &[("<?php", 8), ("$this->", 3), ("namespace ", 3), ("->", 1)],
    ),
    (
        "java",
        "java",
        &[
            ("public class", 5),
            ("import java", 4),
            ("public static void main", 6),
            ("system.out.println", 4),
            ("@override", 4),
            ("new ", 1),
        ],
    ),
    (
        "cpp",
        "c++",
        &[
            ("#include <iostream>", 6),
            ("std::", 4),
            ("template<", 5),
            ("using namespace", 4),
            ("cout <<", 4),
            ("nullptr", 3),
            ("virtual ", 2),
            ("func ", 2),
            ("guard let", 4),
            ("import foundation", 5),
        ],
    ),
    (
        "c",
        "c",
        &[
            ("#include <stdio.h>", 6),
            ("int main(", 4),
            ("printf(", 3),
            ("#include", 3),
            ("malloc(", 3),
            ("struct ", 2),
        ],
    ),
    (
        "cs",
        "c#",
        &[
            ("using system", 5),
            ("namespace ", 2),
            ("public static void main", 6),
            ("console.writeline", 4),
            ("get; set;", 4),
        ],
    ),
    (
        "js",
        "javascript",
        &[
            ("console.log", 4),
            ("=>", 2),
            ("function ", 2),
            ("const ", 2),
            ("require(", 2),
            ("module.exports", 4),
            ("document.", 2),
            ("export default", 4),
        ],
    ),
    (
        "jsx",
        "jsx",
        &[
            ("return (", 3),
            ("classname=", 5),
            ("<div", 4),
            ("usestate", 5),
            ("onclick", 4),
        ],
    ),
    (
        "sh",
        "shell",
        &[
            ("#!/bin/bash", 6),
            ("#!/bin/sh", 6),
            ("fi", 2),
            ("esac", 3),
            ("export ", 2),
            ("$1", 2),
            ("write-host", 5),
            ("$env:", 5),
            ("from ", 1),
            ("run ", 2),
        ],
    ),
    (
        "sql",
        "sql",
        &[
            ("select ", 4),
            ("from ", 3),
            ("where ", 3),
            ("join ", 3),
            ("insert into", 5),
            ("create table", 5),
        ],
    ),
    (
        "html",
        "html",
        &[
            ("<!doctype html", 7),
            ("<html", 4),
            ("<div", 3),
            ("<head>", 3),
            ("<body", 3),
            ("{{ ", 2),
            ("{% ", 2),
        ],
    ),
    (
        "xml",
        "xml",
        &[("<?xml", 8), ("encoding=", 3), ("<project", 3)],
    ),
    (
        "css",
        "css",
        &[
            ("color:", 3),
            ("margin:", 3),
            ("padding:", 3),
            ("display:", 3),
            ("font-size", 3),
            ("@mixin", 5),
            ("&:", 3),
            ("@var", 4),
        ],
    ),
    (
        "yaml",
        "yaml",
        &[
            ("version:", 2),
            ("services:", 4),
            ("apiversion", 5),
            ("kind:", 4),
            ("runs-on", 4),
            ("[package]", 5),
            ("[dependencies]", 5),
            ("resource \"", 6),
            ("variable \"", 5),
        ],
    ),
    (
        "json",
        "json",
        &[
            ("{\n", 2),
            ("\": ", 3),
            ("\":{", 3),
            ("\"name\":", 3),
            ("\"version\":", 4),
        ],
    ),
    (
        "properties",
        "ini",
        &[("[section]", 4), ("[", 2), ("=", 1), (";", 1)],
    ),
    (
        "lua",
        "lua",
        &[
            ("local ", 3),
            ("function(", 4),
            ("then", 2),
            ("end", 2),
            ("require(", 3),
            ("nil", 2),
        ],
    ),
    (
        "hs",
        "haskell",
        &[
            ("module ", 4),
            ("where", 2),
            ("<-|", 4),
            ("::", 3),
            ("import qualified", 4),
            ("data ", 2),
            ("view =", 4),
        ],
    ),
    (
        "ml",
        "ocaml",
        &[
            ("let rec", 5),
            ("match ", 3),
            ("with", 2),
            ("->", 2),
            (";;", 3),
            ("open ", 2),
        ],
    ),
    (
        "lisp",
        "lisp",
        &[
            ("(defun", 6),
            ("(let", 4),
            ("(lambda", 5),
            ("(require", 4),
            ("(setq", 4),
            (":- ", 4),
        ],
    ),
    (
        "scm",
        "scheme",
        &[("(define", 5), ("(display", 4), ("(import", 3)],
    ),
    (
        "clj",
        "clojure",
        &[("(ns ", 6), ("(defn", 5), ("(let [", 4), ("(require", 3)],
    ),
    (
        "scala",
        "scala",
        &[
            ("object ", 4),
            ("def ", 2),
            ("val ", 2),
            ("extends ", 3),
            ("case class", 5),
        ],
    ),
    (
        "erl",
        "erlang",
        &[
            ("-module(", 7),
            ("-export(", 6),
            ("-record(", 6),
            ("receive", 4),
            ("defmodule", 6),
            ("defp ", 4),
            ("|> ", 4),
            ("@moduledoc", 5),
        ],
    ),
    (
        "R",
        "r",
        &[
            ("<-", 4),
            ("function(", 4),
            ("library(", 4),
            ("%>%", 4),
            ("using ", 4),
            ("mutable struct", 5),
        ],
    ),
    (
        "matlab",
        "matlab",
        &[
            ("function y =", 6),
            ("zeros(", 3),
            ("disp(", 3),
            ("end\n", 2),
            ("using ", 3),
            ("::", 3),
            ("struct ", 3),
        ],
    ),
    (
        "groovy",
        "groovy",
        &[
            ("@grab", 6),
            ("println ", 3),
            ("plugins {", 5),
            ("dependencies {", 5),
            ("repositories {", 5),
        ],
    ),
    (
        "pl",
        "perl",
        &[
            ("use strict", 6),
            ("my $", 5),
            ("my @", 5),
            ("sub ", 3),
            ("=head", 4),
        ],
    ),
    (
        "pas",
        "pascal",
        &[
            ("program ", 5),
            ("begin", 4),
            ("end.", 5),
            ("uses ", 4),
            ("procedure ", 4),
            ("package body", 5),
        ],
    ),
    (
        "proto",
        "protobuf",
        &[
            ("syntax = \"proto3\"", 8),
            ("message ", 5),
            ("repeated ", 4),
            ("rpc ", 5),
        ],
    ),
    (
        "graphql",
        "graphql",
        &[
            ("query ", 4),
            ("mutation ", 5),
            ("type query", 6),
            ("schema {", 6),
        ],
    ),
    (
        "bat",
        "batch",
        &[("@echo off", 7), ("goto ", 4), ("if exist", 5)],
    ),
    (
        "make",
        "makefile",
        &[
            (".phony:", 6),
            ("all:", 3),
            ("$(", 3),
            ("clean:", 3),
            ("cmake_minimum_required", 8),
            ("add_executable", 6),
            ("target_link_libraries", 6),
        ],
    ),
    (
        "m",
        "objective-c",
        &[
            ("#import <foundation", 7),
            ("@interface", 7),
            ("@implementation", 7),
            ("nsstring", 4),
        ],
    ),
    (
        "js",
        "typescript",
        &[
            ("interface ", 6),
            ("enum ", 5),
            ("implements ", 4),
            ("readonly ", 4),
            ("abstract class", 5),
            ("namespace ", 2),
            (": number", 4),
            (": void", 3),
            ("<T>", 3),
            ("as const", 4),
        ],
    ),
    (
        "java",
        "kotlin",
        &[
            ("fun main", 6),
            ("data class", 6),
            ("val ", 3),
            ("companion object", 5),
            ("when (", 3),
            ("?:", 1),
        ],
    ),
    (
        "cpp",
        "swift",
        &[
            ("func ", 3),
            ("guard let", 5),
            ("import foundation", 6),
            ("@objc", 4),
            ("deinit", 5),
            ("var ", 2),
        ],
    ),
    (
        "matlab",
        "julia",
        &[
            ("using ", 5),
            ("mutable struct", 6),
            ("struct ", 3),
            ("@enum", 5),
            ("::", 2),
        ],
    ),
    (
        "rb",
        "crystal",
        &[("property ", 5), ("macro ", 5), ("require \"", 3)],
    ),
    (
        "sh",
        "powershell",
        &[
            ("write-host", 6),
            ("$env:", 6),
            ("param(", 5),
            ("get-childitem", 6),
            ("foreach", 4),
            ("$", 1),
        ],
    ),
    (
        "css",
        "scss",
        &[
            ("@mixin", 6),
            ("&:", 4),
            ("@include", 5),
            ("@extend", 5),
            ("@function", 5),
            ("@mixin", 6),
        ],
    ),
    (
        "yaml",
        "toml",
        &[
            ("[package]", 7),
            ("[dependencies]", 7),
            ("[profile", 6),
            ("[tool.", 6),
            ("[workspace]", 6),
        ],
    ),
    (
        "html",
        "vue",
        &[
            ("{{ ", 3),
            ("v-", 4),
            ("<template", 4),
            ("props:", 4),
            ("export default", 3),
        ],
    ),
    (
        "erl",
        "elixir",
        &[
            ("defmodule", 7),
            ("defp ", 5),
            ("|> ", 5),
            ("@moduledoc", 6),
        ],
    ),
    (
        "yaml",
        "terraform",
        &[
            ("resource \"", 8),
            ("variable \"", 7),
            ("provider \"", 6),
            ("terraform {", 7),
            ("output \"", 5),
        ],
    ),
    (
        "html",
        "handlebars",
        &[("{{#", 7), ("{{/", 6), ("{{else", 5)],
    ),
    (
        "html",
        "jinja",
        &[("{% extends", 6), ("{% block", 6), ("{{ ", 2)],
    ),
    (
        "html",
        "twig",
        &[("{% extends", 6), ("{% block", 5), ("do ", 1)],
    ),
    ("html", "erb", &[("<%", 5), ("<%=", 5)]),
    (
        "make",
        "cmake",
        &[
            ("cmake_minimum_required", 8),
            ("add_executable", 7),
            ("target_link_libraries", 7),
            ("find_package(", 6),
            ("project(", 6),
        ],
    ),
    (
        "sh",
        "dockerfile",
        &[
            ("from ", 6),
            ("run ", 5),
            ("cmd ", 5),
            ("entrypoint", 5),
            ("expose ", 6),
            ("workdir ", 6),
            ("copy ", 3),
        ],
    ),
    (
        "matlab",
        "objective-c",
        &[
            ("@interface", 7),
            ("@implementation", 7),
            ("#import <foundation", 8),
            ("nsstring", 4),
        ],
    ),
    (
        "tex",
        "latex",
        &[
            ("\\documentclass", 8),
            ("\\begin{document}", 8),
            ("\\usepackage", 6),
            ("\\section", 5),
            ("@article{", 7),
            ("@book{", 7),
        ],
    ),
];

pub struct Highlighter {
    syntax_set: syntect::parsing::SyntaxSet,
    syntect_theme: syntect::highlighting::Theme,
    md: crate::preview::MdColors,
}

impl Highlighter {
    pub fn new(theme: &opaline::Theme) -> Self {
        let mut builder = syntect::parsing::SyntaxSet::load_defaults_newlines().into_builder();
        // user grammars: drop any sublime-syntax or tmLanguage file into
        // $XDG_CONFIG_HOME/blur/syntaxes and it is picked up at startup.
        if let Some(dir) = user_syntax_dir() {
            let _ = builder.add_from_folder(dir, true);
        }
        let syntect_theme = opaline::adapters::syntect::to_syntect_theme(theme);
        Highlighter {
            syntax_set: builder.build(),
            syntect_theme,
            md: md_colors(theme),
        }
    }

    /// cheap theme swap for live picker preview: syntaxes stay loaded,
    /// only the syntect theme rebuilds. callers must clear tab caches.
    pub fn set_theme(&mut self, theme: &opaline::Theme) {
        self.syntect_theme = opaline::adapters::syntect::to_syntect_theme(theme);
        self.md = md_colors(theme);
    }

    fn by_ext(&self, ext: &str) -> Option<&syntect::parsing::SyntaxReference> {
        let ext = ext.trim_start_matches('.').to_lowercase();
        if ext.is_empty() {
            return None;
        }
        if let Some(s) = self.syntax_set.find_syntax_by_extension(&ext) {
            return Some(s);
        }
        // one hop through the alias table, so ts -> js -> javascript
        if let Some((_, target)) = EXT_ALIASES.iter().find(|(k, _)| *k == ext) {
            return self.syntax_set.find_syntax_by_extension(target);
        }
        None
    }

    /// resolve a bare name such as "rust" or "js" to a grammar
    fn by_name(&self, name: &str) -> Option<&syntect::parsing::SyntaxReference> {
        let n = name.trim().to_lowercase();
        self.by_ext(&n)
    }

    /// path based detection. syntect handles the common name list, we add
    /// ours and then disambiguate the extensions it gets wrong.
    fn detect_by_path(&self, path: &str, body: &str) -> Option<&syntect::parsing::SyntaxReference> {
        let lower = path.to_lowercase();
        let base = lower.rsplit('/').next().unwrap_or(&lower).to_string();

        // exact whole-file names first, they carry no extension
        if let Some((_, target)) = NAME_ALIASES.iter().find(|(k, _)| *k == base)
            && let Some(s) = self.by_name(target)
        {
            return Some(s);
        }
        if let Ok(Some(s)) = self.syntax_set.find_syntax_for_file(&lower) {
            return Some(self.disambiguate(path, s, body));
        }
        // compound extensions such as .d.ts, .blade.php, .tar.gz handled
        // by walking suffixes from the longest to the shortest.
        let mut parts: Vec<&str> = base.split('.').collect();
        if parts.len() > 2 {
            parts.remove(0);
            let joined = parts.join(".");
            if let Ok(Some(s)) = self.syntax_set.find_syntax_for_file(&joined) {
                return Some(self.disambiguate(path, s, body));
            }
        }
        if let Some((_, ext)) = base.rsplit_once('.')
            && let Some(s) = self.by_ext(ext)
        {
            return Some(self.disambiguate(path, s, body));
        }
        None
    }

    /// syntect maps some extensions to the wrong family. use the body of
    /// the file to pick the right one.
    fn disambiguate<'a>(
        &'a self,
        path: &str,
        syntax: &'a syntect::parsing::SyntaxReference,
        body: &str,
    ) -> &'a syntect::parsing::SyntaxReference {
        let ext = path
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            // .h and .m are shared by several languages
            "h" | "hh" | "hpp" | "hxx" => {
                let body = self.evidence(path, body);
                if body.contains("namespace ")
                    || body.contains("template<")
                    || body.contains("class ")
                    || body.contains("public:")
                    || body.contains("private:")
                    || body.contains("protected:")
                {
                    return self.by_ext("cpp").unwrap_or(syntax);
                }
                // syntect always answers Objective-C for .h, which is
                // wrong for the far more common C header
                if body.contains("#import")
                    || body.contains("@interface")
                    || body.contains("@implementation")
                {
                    return self.by_ext("m").unwrap_or(syntax);
                }
                self.by_ext("c").unwrap_or(syntax)
            }
            "m" => {
                let body = self.evidence(path, body);
                if body.contains("#import") || body.contains("@interface") {
                    return self.by_ext("m").unwrap_or(syntax);
                }
                if body.contains("function") && body.contains("end") {
                    return self.by_ext("matlab").unwrap_or(syntax);
                }
                syntax
            }
            // R is case sensitive and .r collides with Rust tooling
            "r" => {
                let body = self.evidence(path, body);
                if body.contains("<-") || body.contains("library(") {
                    return self.by_ext("r").unwrap_or(syntax);
                }
                syntax
            }
            _ => syntax,
        }
    }

    /// prefer the buffer we already have, fall back to the file on disk
    /// when the buffer is empty or has not been saved yet.
    fn evidence(&self, path: &str, body: &str) -> String {
        let from_buffer: String = body.chars().take(4096).collect();
        if from_buffer.trim().is_empty() {
            return self.quick_peek(path);
        }
        from_buffer
    }

    /// tiny on disk peek, only used to break extension ties
    fn quick_peek(&self, path: &str) -> String {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.chars().take(4096).collect())
            .unwrap_or_default()
    }

    fn detect_by_shebang(
        &self,
        first: &str,
    ) -> Option<(&syntect::parsing::SyntaxReference, &'static str)> {
        let line = first.trim();
        if !line.starts_with("#!") {
            return None;
        }
        // take the first token of the interpreter, handling env -S and
        // absolute or bare interpreter names alike
        let rest = line.trim_start_matches("#!").trim();
        let mut words = rest
            .split_whitespace()
            .filter(|w| !w.starts_with('-') || w.len() == 1)
            .peekable();
        let mut token = words.next().unwrap_or("");
        if token.ends_with("env") {
            token = words.next().unwrap_or("");
        } else if let Some(idx) = token.rfind('/') {
            token = &token[idx + 1..];
        }
        let token = token.trim_start_matches('-');
        if token.is_empty() {
            return None;
        }
        for (interp, ext) in SHEBANGS {
            if token.eq_ignore_ascii_case(interp) {
                return self.by_name(ext).map(|s| (s, *interp));
            }
        }
        // interpreter with a version suffix, python3.12 -> python
        let base: String = token
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        for (interp, ext) in SHEBANGS {
            if base.eq_ignore_ascii_case(interp) {
                return self.by_name(ext).map(|s| (s, *interp));
            }
        }
        None
    }

    /// score the body against every signature and take the best match
    /// that clears the noise floor. returns the grammar plus the language
    /// name to show, since several languages share one bundled grammar.
    fn detect_by_content(
        &self,
        low: &str,
    ) -> Option<(&syntect::parsing::SyntaxReference, &'static str)> {
        if low.trim().is_empty() {
            return None;
        }
        let mut ranked: Vec<(&str, &'static str, u32)> = SIGNATURES
            .iter()
            .map(|(ext, label, sigs)| {
                let score = sigs
                    .iter()
                    .filter(|(needle, _)| low.contains(needle))
                    .map(|(_, w)| *w)
                    .sum::<u32>();
                (*ext, *label, score)
            })
            .filter(|(_, _, score)| *score > 0)
            .collect();
        ranked.sort_by_key(|r| std::cmp::Reverse(r.2));
        let (ext, label, score) = *ranked.first()?;
        // a clear winner clears a lower floor, a close race needs a
        // stronger signal so common words cannot pick a language
        let runner_up = ranked.get(1).map(|(_, _, s)| *s).unwrap_or(0);
        if score < 3 || (score < 5 && score <= runner_up) {
            return None;
        }
        let syntax = self.by_ext(ext)?;
        Some((syntax, label))
    }

    /// layered detection: path, then shebang, then first line, then body.
    /// detection returns both the grammar and the name to show
    fn detect_syntax_parts(
        &self,
        tab: &Tab,
        low: &str,
    ) -> (&syntect::parsing::SyntaxReference, String) {
        if !tab.file_name.is_empty()
            && let Some(s) = self.detect_by_path(&tab.file_name, low)
        {
            let label = self
                .path_label(&tab.file_name, s)
                .unwrap_or_else(|| grammar_label(s));
            return (s, label);
        }
        let first = tab.input_box.first().map(String::as_str).unwrap_or("");
        if !first.is_empty() {
            if let Some((s, interp)) = self.detect_by_shebang(first) {
                return (s, interp.to_string());
            }
            if let Some(s) = self.syntax_set.find_syntax_by_first_line(first) {
                return (s, grammar_label(s));
            }
        }
        if let Some((s, label)) = self.detect_by_content(low) {
            return (s, label.to_string());
        }
        let plain = self.syntax_set.find_syntax_plain_text();
        (plain, "text".to_string())
    }

    /// language name for the status bar. aliased languages report
    /// themselves, so a .ts file reads "typescript" while still being
    /// highlighted with the javascript grammar.
    pub fn language(&self, tab: &Tab) -> String {
        if !tab.lang.is_empty() {
            return tab.lang.clone();
        }
        let low = content_sample(&tab.input_box);
        self.detect_syntax_parts(tab, &low).1
    }

    /// label implied by the file extension alone
    fn path_label(&self, path: &str, syntax: &syntect::parsing::SyntaxReference) -> Option<String> {
        let base = path.rsplit('/').next().unwrap_or(path).to_lowercase();
        if let Some((_, label)) = NAME_LABELS.iter().find(|(n, _)| *n == base) {
            return Some((*label).to_string());
        }
        let ext = base
            .rsplit_once('.')
            .map(|(_, e)| e.to_string())
            .unwrap_or(base);
        let alias_label = ALIAS_LABELS
            .iter()
            .find(|(alias, _)| *alias == ext.as_str())
            .map(|(_, label)| *label);
        let target = EXT_ALIASES
            .iter()
            .find(|(alias, _)| *alias == ext.as_str())
            .map(|(_, t)| *t);
        if let (Some(label), Some(target)) = (alias_label, target)
            && let Some(grammar) = self.by_ext(target)
            && grammar.name == syntax.name
        {
            return Some(label.to_string());
        }
        None
    }
}

/// friendly name for a bundled grammar
fn grammar_label(syntax: &syntect::parsing::SyntaxReference) -> String {
    if syntax.name == "Plain Text" {
        "text".to_string()
    } else {
        syntax.name.to_lowercase()
    }
}

/// lowercase sample of the buffer used by content sniffing
fn content_sample(lines: &[String]) -> String {
    lines
        .iter()
        .take(80)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
}

fn user_syntax_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("blur").join("syntaxes"))
}

fn md_colors(theme: &opaline::Theme) -> crate::preview::MdColors {
    use ratatui::style::Color;
    let c = |v: opaline::OpalineColor| Color::Rgb(v.r, v.g, v.b);
    crate::preview::MdColors {
        text: c(tok(theme, "text.primary")),
        dim: c(tok(theme, "text.dim")),
        faint: c(tok(theme, "border.unfocused")),
        accent: c(tok(theme, "accent.deep")),
        code: c(hue(theme, "green")),
        quote: c(tok(theme, "text.dim")),
    }
}

impl Highlighter {
    /// populate the cache if it is stale, then hand back a borrow of it.
    /// the renderer styles from this directly, so no whole-file clone
    /// happens on every frame.
    pub fn ensure_fresh<'a>(&'a self, tab: &'a mut Tab) -> &'a Vec<ratatui::text::Line<'a>> {
        if tab.highlight_cache.is_none() {
            self.compute(tab);
        }
        tab.highlight_cache.as_ref().expect("just populated")
    }

    fn compute(&self, tab: &mut Tab) {
        let low = content_sample(&tab.input_box);
        let (syntax, label) = self.detect_syntax_parts(tab, &low);
        tab.lang = label;
        // syntect themes carry no markup scopes, so markdown gets a
        // dedicated pass: real colors in the editor, not flat text.
        if syntax.name == "Markdown" {
            let mut out = Vec::new();
            let mut fence = false;
            for line in &tab.input_box {
                if line.trim_start().starts_with("```") {
                    fence = !fence;
                    out.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            line.to_string(),
                            ratatui::style::Style::default().fg(self.md.dim),
                        ),
                    ]));
                    continue;
                }
                if fence {
                    out.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            line.to_string(),
                            ratatui::style::Style::default().fg(self.md.code),
                        ),
                    ]));
                    continue;
                }
                let t = line.trim_start();
                let hashes = t.chars().take_while(|c| *c == '#').count();
                if hashes > 0 && t[hashes..].starts_with(' ') {
                    let mut spans = vec![ratatui::text::Span::styled(
                        " ".repeat(line.len() - t.len()) + &"#".repeat(hashes) + " ",
                        ratatui::style::Style::default().fg(self.md.dim),
                    )];
                    spans.extend(crate::preview::inline(
                        t[hashes + 1..].trim_start(),
                        &self.md,
                    ));
                    // headings read bold without any background wash
                    for s in spans.iter_mut().skip(1) {
                        s.style = s.style.add_modifier(ratatui::style::Modifier::BOLD);
                    }
                    out.push(ratatui::text::Line::from(spans));
                    continue;
                }
                out.push(ratatui::text::Line::from(crate::preview::inline(
                    line, &self.md,
                )));
            }
            tab.highlight_cache = Some(out);
            return;
        }

        let mut the_highlighter = syntect::easy::HighlightLines::new(syntax, &self.syntect_theme);
        let mut spans: Vec<ratatui::text::Line> = Vec::new();

        for line in &tab.input_box {
            // the newline grammar set matches on the line terminator, so
            // it has to be fed in. without it a comment or string scope
            // never closes and swallows every following line.
            let mut owned = String::with_capacity(line.len() + 1);
            owned.push_str(line);
            if !line.ends_with('\n') {
                owned.push('\n');
            }
            let range = the_highlighter
                .highlight_line(&owned, &self.syntax_set)
                .unwrap_or_default();
            let mut spans_for_line: Vec<ratatui::text::Span> = Vec::new();
            for (style, text) in range {
                let fg = style.foreground;
                let text = text.trim_end_matches('\n');
                if text.is_empty() {
                    continue;
                }
                spans_for_line.push(ratatui::text::Span::styled(
                    text.to_string(),
                    ratatui::style::Style::default()
                        .fg(ratatui::style::Color::Rgb(fg.r, fg.g, fg.b)),
                ));
            }
            spans.push(ratatui::text::Line::from(spans_for_line));
        }
        tab.highlight_cache = Some(spans);
    }
}

pub struct Tab {
    pub file_name: String,
    /// bumped on every edit, lets the preview skip re-parsing a
    /// document that has not changed
    pub revision: u64,
    /// language shown in the status bar, filled in while highlighting
    pub lang: String,
    pub saved: bool,
    pub highlight_cache: Option<Vec<ratatui::text::Line<'static>>>,
    pub input_box: Vec<String>,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub scroll_x: u16,
    pub scroll_y: u16,
    pub undo_stack: Vec<EditRecord>,
    pub redo_stack: Vec<EditRecord>,
}

impl Tab {
    pub fn new() -> Self {
        Self {
            file_name: String::from(""),
            revision: 0,
            lang: String::new(),
            saved: false,
            highlight_cache: None,
            input_box: vec![String::new()],
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            scroll_x: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn unsave(&mut self) {
        self.saved = false;
        self.highlight_cache = None;
        self.revision = self.revision.wrapping_add(1);
    }
}

pub enum EditRecord {
    DeleteChar {
        row: usize,
        col: usize,
        ch: char,
    },
    InsertString {
        row: usize,
        col: usize,
        text: String,
    },
    RemoveString {
        row: usize,
        col: usize,
        text: String,
    },
    SplitLine {
        row: usize,
        col: usize,
    },
    /// enter between `{` and `}` (or `[]`, `()`): one keypress makes
    /// three lines. single undo unit via stored tail.
    BraceSplit {
        row: usize,
        col: usize,
        indent: String,
        base: String,
        tail: String,
    },
    MergeLine {
        row: usize,
        prev_len: usize,
    },
    InsertLine {
        row: usize,
    },
    RemoveLine {
        row: usize,
        content: String,
    },
    RemoveEmptyLine {
        row: usize,
    },
}

pub fn apply_inverse(record: &EditRecord, input_box: &mut Vec<String>) -> (usize, usize) {
    match record {
        EditRecord::DeleteChar { row, col, ch } => {
            input_box[*row].insert(*col, *ch);
            (*row, *col + ch.len_utf8())
        }

        EditRecord::InsertString { row, col, text } => {
            let end = col + text.len();
            input_box[*row].replace_range(*col..end, "");
            (*row, *col)
        }

        EditRecord::RemoveString { row, col, text } => {
            input_box[*row].insert_str(*col, text);
            (*row, *col)
        }

        EditRecord::SplitLine { row, col } => {
            let next_line = input_box.remove(*row + 1);
            input_box[*row].push_str(&next_line);
            (*row, *col)
        }

        EditRecord::BraceSplit { row, col, tail, .. } => {
            input_box.remove(*row + 2);
            input_box.remove(*row + 1);
            input_box[*row].push_str(tail);
            (*row, *col)
        }

        EditRecord::MergeLine { row, prev_len } => {
            let combined = input_box[*row - 1].split_off(*prev_len);
            input_box.insert(*row, combined);
            (*row, 0)
        }

        EditRecord::InsertLine { row } => {
            input_box.remove(*row);
            (*row, 0)
        }

        EditRecord::RemoveLine { row, content } => {
            input_box.insert(*row, content.clone());
            (*row, 0)
        }
        EditRecord::RemoveEmptyLine { row } => {
            input_box.insert(*row, String::new());
            (*row, 0)
        }
    }
}

pub fn apply_forward(record: &EditRecord, input_box: &mut Vec<String>) -> (usize, usize) {
    match record {
        EditRecord::DeleteChar { row, col, .. } => {
            input_box[*row].remove(*col);
            (*row, *col)
        }

        EditRecord::InsertString { row, col, text } => {
            input_box[*row].insert_str(*col, text);
            (*row, *col + text.len())
        }

        EditRecord::RemoveString { row, col, text } => {
            let end = col + text.len();
            input_box[*row].replace_range(*col..end, "");
            (*row, *col)
        }

        EditRecord::SplitLine { row, col } => {
            let rest = input_box[*row].split_off(*col);
            input_box.insert(*row + 1, rest);
            (*row + 1, 0)
        }

        EditRecord::BraceSplit {
            row,
            col,
            indent,
            base,
            tail,
        } => {
            let head_len = input_box[*row].len().saturating_sub(tail.len());
            let col = (*col).min(head_len);
            input_box[*row].truncate(col);
            input_box.insert(*row + 1, indent.clone());
            input_box.insert(*row + 2, format!("{base}{tail}"));
            (*row + 1, indent.len())
        }

        EditRecord::MergeLine { row, prev_len } => {
            let combined = input_box.remove(*row + 1);
            input_box[*row].push_str(&combined);
            (*row, *prev_len)
        }

        EditRecord::InsertLine { row } => {
            input_box.insert(*row, String::new());
            (*row, 0)
        }
        EditRecord::RemoveLine { row, .. } => {
            input_box.remove(*row);
            (*row, 0)
        }

        EditRecord::RemoveEmptyLine { row } => {
            input_box.remove(*row);
            (*row, 0)
        }
    }
}

/// debug logger for a fullscreen app where stdout is unusable.
/// writes to blur-log.txt in the working directory.
#[allow(dead_code)]
pub fn log(msg: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("blur-log.txt")
    {
        writeln!(file, "{}", msg).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hl() -> Highlighter {
        let theme = opaline::load_by_name("catppuccin-mocha").unwrap();
        Highlighter::new(&theme)
    }

    fn lang_for_file(name: &str, body: &str) -> String {
        let h = hl();
        let mut tab = Tab::new();
        tab.file_name = name.to_string();
        tab.input_box = body.split('\n').map(str::to_string).collect();
        h.language(&tab)
    }

    fn lang_for_body(body: &str) -> String {
        lang_for_file("", body)
    }

    #[test]
    fn extensions_map_to_the_right_family() {
        for (name, want) in [
            ("main.rs", "rust"),
            ("app.py", "python"),
            ("a.go", "go"),
            ("a.rb", "ruby"),
            ("a.lua", "lua"),
            ("a.hs", "haskell"),
            ("a.ml", "ocaml"),
            ("a.clj", "clojure"),
            ("a.scala", "scala"),
            ("a.ex", "elixir"),
            ("a.php", "php"),
            ("a.java", "java"),
            ("a.cs", "c#"),
            ("a.sql", "sql"),
            ("a.yaml", "yaml"),
            ("a.json", "json"),
            ("a.md", "markdown"),
            ("a.toml", "toml"),
            ("a.scss", "scss"),
        ] {
            let got = lang_for_file(name, "x");
            assert_eq!(got, want, "for {name}");
        }
    }

    #[test]
    fn extensions_without_a_bundled_grammar_land_on_a_neighbour() {
        for (name, want) in [
            ("a.ts", "typescript"),
            ("a.tsx", "tsx"),
            ("a.kt", "kotlin"),
            ("a.swift", "swift"),
            ("a.jl", "julia"),
            ("a.zig", "zig"),
            ("a.cr", "crystal"),
            ("a.vue", "vue"),
            ("a.ps1", "powershell"),
            ("a.proto", "protobuf"),
            ("a.tf", "terraform"),
            ("a.vue", "vue"),
        ] {
            let got = lang_for_file(name, "x");
            assert_eq!(got, want, "for {name}");
        }
    }

    #[test]
    fn whole_file_names_are_recognised() {
        for (name, want) in [
            ("Makefile", "makefile"),
            ("Dockerfile", "dockerfile"),
            ("Rakefile", "ruby"),
            ("Gemfile", "ruby"),
            ("Cargo.toml", "toml"),
            (".gitignore", "gitignore"),
        ] {
            let got = lang_for_file(name, "x");
            assert_eq!(got, want, "for {name}");
        }
    }

    #[test]
    fn shebangs_win_for_extensionless_files() {
        assert_eq!(lang_for_body("#!/usr/bin/env python3\nprint(1)"), "python");
        assert_eq!(lang_for_body("#!/bin/bash\necho hi"), "bash");
        assert_eq!(lang_for_body("#!/usr/bin/env node\nconsole.log(1)"), "node");
        assert_eq!(lang_for_body("#!/usr/bin/perl -w\nmy $x = 1;"), "perl");
        assert_eq!(
            lang_for_body("#!/usr/bin/env -S deno run\nexport {}"),
            "deno"
        );
        assert_eq!(lang_for_body("#!/usr/bin/env ruby\nputs 1"), "ruby");
    }

    #[test]
    fn untitled_buffers_are_sniffed_from_content() {
        assert_eq!(lang_for_body("fn main() {\n    let mut x = 1;\n}"), "rust");
        assert_eq!(lang_for_body("package main\nfunc main() {}"), "go");
        assert_eq!(lang_for_body("def greet(name):\n    return name"), "python");
        assert_eq!(lang_for_body("<?php\necho 'hi';"), "php");
        assert_eq!(
            lang_for_body("public static void main(String[] a) {}"),
            "java"
        );
        assert_eq!(lang_for_body("interface Foo { bar: string }"), "typescript");
        assert_eq!(
            lang_for_body("#include <iostream>\nint main() { std::cout << 1; }"),
            "c++"
        );
        assert_eq!(
            lang_for_body("-module(main).\n-export([main/0])."),
            "erlang"
        );
        assert_eq!(
            lang_for_body("defmodule Foo do\n  def bar, do: 1\nend"),
            "elixir"
        );
        assert_eq!(
            lang_for_body("FROM ubuntu:22.04\nRUN apt-get update"),
            "dockerfile"
        );
        assert_eq!(
            lang_for_body("[package]\nname = \"x\"\nversion = \"0.1.0\""),
            "toml"
        );
        assert_eq!(lang_for_body("fun main() {\n  val x = 1\n}"), "kotlin");
    }

    #[test]
    fn comment_does_not_swallow_the_rest_of_the_file() {
        // the newline grammar set matches on the line terminator. feeding
        // lines without it left comment scopes open forever and painted
        // every following line with the comment color.
        let h = hl();
        let mut tab = Tab::new();
        tab.file_name = "x.py".into();
        tab.input_box = vec![
            "#!/usr/bin/env python3".into(),
            String::new(),
            "import asyncio".into(),
            "x = 42  # note".into(),
            "def greet(name):".into(),
            "    return name".into(),
        ];
        let lines = h.ensure_fresh(&mut tab).clone();
        let color_of = |li: usize, ci: usize| -> (u8, u8, u8) {
            match lines[li].spans[ci].style.fg {
                Some(ratatui::style::Color::Rgb(r, g, b)) => (r, g, b),
                _ => (0, 0, 0),
            }
        };
        // the import line and the keyword after it must not share the
        // comment color of line 0
        assert_ne!(
            color_of(2, 0),
            color_of(0, 0),
            "import line took comment color"
        );
        assert_ne!(color_of(4, 0), color_of(0, 0), "def took comment color");
    }

    #[test]
    fn empty_and_unknown_input_falls_back_to_text() {
        assert_eq!(lang_for_body(""), "text");
        assert_eq!(lang_for_body("zzz qqq\njust some prose here"), "text");
    }

    #[test]
    fn ambiguous_headers_resolve_by_body() {
        let h = hl();
        // a .h with classes is c++, otherwise c
        let mut t = Tab::new();
        t.file_name = "x.h".into();
        t.input_box = vec!["class Foo {};".into()];
        let got = h
            .detect_by_path("x.h", "class Foo {};")
            .map(|s| s.name.clone());
        assert_eq!(got.as_deref(), Some("C++"));
        let mut t2 = Tab::new();
        t2.file_name = "y.h".into();
        t2.input_box = vec!["int add(int a, int b) { return a + b; }".into()];
        assert_eq!(
            h.detect_by_path("y.h", "int add(int a) { return a; }")
                .map(|s| s.name.clone())
                .as_deref(),
            Some("C")
        );
    }
}
