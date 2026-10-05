#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const commander_1 = require("commander");
const init_1 = require("./commands/init");
const codegen_1 = require("./commands/codegen");
const build_1 = require("./commands/build");
const add_1 = require("./commands/add");
const validate_1 = require("./commands/validate");
const export_values_1 = require("./commands/export_values");
const test_1 = require("./commands/test");
const program = new commander_1.Command();
program
    .name("logrix")
    .description("Developer CLI for Logrix blockchain indexers")
    .version("0.3.0");
program.addCommand((0, init_1.initCommand)());
program.addCommand((0, add_1.addCommand)());
program.addCommand((0, codegen_1.codegenCommand)());
program.addCommand((0, validate_1.validateCommand)());
program.addCommand((0, build_1.buildCommand)());
program.addCommand((0, export_values_1.exportValuesCommand)());
program.addCommand((0, test_1.testCommand)());
program.parse(process.argv);
