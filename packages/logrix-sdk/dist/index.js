#!/usr/bin/env node
"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const commander_1 = require("commander");
const init_1 = require("./commands/init");
const codegen_1 = require("./commands/codegen");
const build_1 = require("./commands/build");
const program = new commander_1.Command();
program
    .name("logrix")
    .description("Developer CLI for Logrix blockchain indexers")
    .version("0.1.0");
program.addCommand((0, init_1.initCommand)());
program.addCommand((0, codegen_1.codegenCommand)());
program.addCommand((0, build_1.buildCommand)());
program.parse(process.argv);
