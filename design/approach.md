                           Tarpaulin

1.  Introduction

  Tarpaulin is a proposed coverage tool to calculate various metrics of rust 
  projects. Code coverage analysis is a difficult problem and therefore some 
  design must be done upfront.

2.  User Interface

  Ideally this tool would have an interface similar to cargo check, test and 
  fmt. Users would go to the root level of their project and type:

    cargo tarpaulin [OPTIONS]

  The question is whether there should be default coverage types and other 
  options or whether the user should specify what coverage types they require.

  Currently the desired functionality is:

  * Help - provides information to the user
  * Line coverage - has each line been executed
  * Branch coverage - has each branch been executed
  * Condition coverage - has each boolean expression been true and false.
  * Output - where should the output data be exported to

  For a minimum working product only one coverage method would need to be 
  addressed. As functionality grows additional extended functions may be added 
  such as:

  * Filters - exclude certain parts of the code from analysis 
  * Function coverage - identify functions not called during testing
  * MCDC coverage - more thorough that branch and condition coverage
  * Service integration - integrate with tools like coveralls and codecov
  * Test gap identification - find an area to test to give the most gains 

3.  Representing the Binary

  Representing the source code and the tests is important as this data structure
  will direct a lot of the design of the parts downstream and how easily 
  different approaches can be developed in future.

3.1.  Naive Approaches

  Naive approaches have the benefit of being easily implemented, however they 
  may hinder design and result in a lot of future rework. Approaches could 
  include:

  * Hashmap of line addresses and coverage statistics of interest
  * The raw binary, using something like ptrace and bespoke logging together

3.2.  Graph Structures

  Source code can be represented as a graph structure which enables easy 
  traversal as well as showing the structural relationship of the code.

  An initial approach could have nodes representing function blocks and directed
  edges showing callers and callees. Some packet strucutre would need to be 
  implemented to represent the data flowing to and from these nodes and to 
  facilitate in node analysis. Then a node and the data packet could be passed
  to an analysis visitor to update the logged statistics.

  Other approaches could see something like an Abstract Syntax Tree (AST) which
  would then represent code at a line or decision level. This could make it 
  harder to identify relationships between callers and callees.

4.  Further Investigation

  The rust compiler may represent the source in some manner before the MIR level
  to aid in analysis of borrowing and other rules. I should investigate how it
  solves it's problems. This may also lead to a compiler plugin as a potential
  solution.

5.  Implementation

  * Look for Cargo.toml in current directory. 
  * Look for /tests/ directory.
  * Log name of all rust files in /tests/
  * Log name of all files in src? (or just the lib/bin name?)
  * Clean project
  * Build tests no-run with dead code.
  * Find test entry points
  * Parse
  * Be happy.
