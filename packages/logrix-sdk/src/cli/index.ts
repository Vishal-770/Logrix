#!/usr/bin/env node
import { Command } from "commander";
import { initCommand } from "./commands/init";
import { codegenCommand } from "./commands/codegen";
import { buildCommand } from "./commands/build";
import { addCommand } from "./commands/add";
import { validateCommand } from "./commands/validate";

const program = new Command();

program
  .name("logrix")
  .description("Developer CLI for Logrix blockchain indexers")
  .version("0.2.0");

program.addCommand(initCommand());
program.addCommand(addCommand());
program.addCommand(codegenCommand());
program.addCommand(validateCommand());
program.addCommand(buildCommand());

program.parse(process.argv);
