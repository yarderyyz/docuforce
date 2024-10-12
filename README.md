## Docuforce

Extract docstrings nag you when they don't match the code!

Currently only supports documenation on rust functions.

## Usage

Set your open ai api key in the terminal environment:

# On macOS/Linux
```
export OPENAI_API_KEY='sk-...'
```

# On Windows Powershell
```
$Env:OPENAI_API_KEY='sk-...'
```

then run:

`docuforce --file <FILE>.rs`

