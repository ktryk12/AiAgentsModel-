$datasetId = "72e7241b-625e-4ca8-853c-70f8bc0cc184"
$url = "http://localhost:8080/training/jobs"
$body = @{
    dataset_id = $datasetId
    base_model_repo = "test/repo"
    base_model_revision = "main"
    name = "recovery-test-job"
    lora = @{
        r = 8
        alpha = 16
        epochs = 1
        lr = 0.0002
    }
} | ConvertTo-Json

# 1. Start Job
Write-Host "Starting Job for Recovery Test..."
try {
    $resp = Invoke-RestMethod -Uri $url -Method Post -Body $body -ContentType "application/json"
    $jobId = $resp.job_id
    Write-Host "Job Created: $jobId"
} catch {
    Write-Error "Failed to create job: $_"
    exit 1
}

# 2. Wait a bit (simulate running)
Start-Sleep -Seconds 3

# 3. Kill Orchestrator
Write-Host "Killing Orchestrator..."
Stop-Process -Name "orchestrator" -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

# 4. Restart Orchestrator
Write-Host "Restarting Orchestrator..."
$proc = Start-Process -FilePath "item" -ArgumentList "cargo run -p orchestrator" -WorkingDirectory "e:\repos\AiAgentsModel-\verifiable-ai" -PassThru -NoNewWindow
# Simple hack: assume running locally in new window or background. 
# Better: Use Start-Job or run command directly.
# Since I am in agent environment, I can just run `cargo run` using tool `run_command` in background after this script finishes? 
# No, I need to restart it *during* this test or split the test.
# Simplest: Just use `taskkill` here, then I (the agent) will restart the orchestrator using `run_command`, then run a 2nd script to check status.

Write-Host "Orchestrator killed. Agent, please restart orchestrator and check status of $jobId"
$jobId | Out-File "last_job_id.txt"
exit 0
