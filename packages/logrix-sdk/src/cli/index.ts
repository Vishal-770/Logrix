#!/usr/bin/env node
import { Command } from "commander";
import { initCommand } from "./commands/init";
import { codegenCommand } from "./commands/codegen";
import { buildCommand } from "./commands/build";

const program = new Command();

program
  .name("logrix")
  .description("Developer CLI for Logrix blockchain indexers")
  .version("0.1.0");

program.addCommand(initCommand());
program.addCommand(codegenCommand());
program.addCommand(buildCommand());

program.parse(process.argv);
