use std::env::consts::OS;
use std::process::Command;

pub fn translate_and_execute(program: &str, args: &[String]) -> Result<Option<String>, String> {
    if OS == "windows" {
        // Windows specific translations
        let (ps_cmd, arg_string) = match program {
            "grep" => ("Select-String", args.join(" ")),
            "curl" | "wget" => ("Invoke-WebRequest", args.join(" ")),
            "find" => ("Get-ChildItem -Recurse", args.join(" ")),
            "clear" | "cls" => ("Clear-Host", String::new()),
            _ => ("", String::new()), // Not translated
        };

        if !ps_cmd.is_empty() {
            let full_command = format!("{} {}", ps_cmd, arg_string);
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg(&full_command);

            let output = cmd.output().map_err(|e| format!("Failed to execute powershell: {}", e))?;
            
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }

            return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
        }
    }

    // If no translation or not windows, we return None to let executor try default behavior
    Ok(None)
}
