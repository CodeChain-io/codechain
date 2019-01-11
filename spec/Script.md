Codechain saves the unlock condition for each asset, and anyone who can pass this condition is authorized to use the asset. The unlock condition is represented in a byte array, and decoded as a list of instructions before execution. Script language is intentionally designed to not be Turing-complete, and all scripts are ensured to be finished in finite time.

# Notation

Assets don’t hold a full script describing its unlock condition, but only a hash of it. We refer to this script as the **lock script**, and the hash as the **script hash**. Before lock script is executed, some predefined values are inserted to stack. These values are called **parameters**, and saved along with script hash. To consume an asset, user must provide **unlock script**, a script which will be executed together with lock script.

Script is encoded as a byte string, and each byte of this string is referred to as a script byte. Instructions are execution units of CCVM, and can be composed of multiple script bytes. The frontmost byte of instruction is the identifier of instruction, and is called opcode.

# CCVM (CodeChain Virtual Machine)

Script language in CodeChain uses [CodeChain Virtual Machine](CodeChain-Virtual-Machine.md) as underlying execution environment.

# Script execution

## Overall process

The overall execution process is similar to P2SH in Bitcoin. Detailed execution process is as follows:

1. Check if an asset’s script hash is equal to the hash of provided lock script.
1. Decode the lock script and unlock script into a list of instructions.
1. Check if unlock script is sane. Currently, it's considered invalid if any opcode other than PUSH-related codes are included.
1. Insert the provided parameters into stack. Order of insertion must be last to first, so that first parameter appears at top of the stack.
1. Execute the unlock script, and then the lock script.

If an exception occurs during the procedure described above, the transaction will be marked as failed.

## Execution result

The result is **SUCCESS** when **all** of the following conditions are met:

* There are no more instructions to execute
* The stack only has one item
* The stack’s topmost value is not zero when converted into an integer

The result is **BURNT** when one of the following conditions are met:
* Self-burning instruction was executed

The result is **FAIL** for all other cases
