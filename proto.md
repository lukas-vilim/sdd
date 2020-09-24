# Supported types 
* floats
* ints
* bools
* strings (string ids)

# Message types
* Table
* Entry
* String

## String
In form of a string table.

* uid -> u32
* len -> u32
* data -> [u8]

## Table
New table request.

* uid -> u32
* num_fields -> u32
* fields
	* type -> u8
	* name -> u32 (string id)

## Entry
New value.

* uid -> 32
* values
	* data -> [u8]
