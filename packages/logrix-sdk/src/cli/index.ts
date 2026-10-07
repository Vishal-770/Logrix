#!/usr/bin/env node
import { Command } from "commander";
import { initCommand } from "./commands/init";
import { codegenCommand } from "./commands/codegen";
import { buildCommand } from "./commands/build";
import { addCommand } from "./commands/add";
import { validateCommand } from "./commands/validate";
import { exportValuesCommand } from "./commands/export_values";
import { testCommand } from "./commands/test";
import { statusCommand } from "./commands/status";

const program = new Command();

program
  .name("logrix")
  .description("Developer CLI for Logrix blockchain indexers")
  .version("0.3.1");

program.addCommand(initCommand());
program.addCommand(addCommand());
program.addCommand(codegenCommand());
program.addCommand(validateCommand());
program.addCommand(buildCommand());
program.addCommand(exportValuesCommand());
program.addCommand(testCommand());
program.addCommand(statusCommand());

program.parse(process.argv);
