$datasetId = "72e7241b-625e-4ca8-853c-70f8bc0cc184"
$url = "http://localhost:8080/training/jobs"
$body = @{
    dataset_id = $datasetId
    base_model_repo = "test/repo"
    base_model_revision = "main"
    name = "verif-job-1"
    lora = @{
        r = 8
        alpha = 16
        epochs = 1
        lr = 0.0002
    }
} | ConvertTo-Json

# 1. Create Job
Write-Host "Creating Job..."
try {
    $resp = Invoke-RestMethod -Uri $url -Method Post -Body $body -ContentType "application/json"
    $jobId = $resp.job_id
    Write-Host "Job Created: $jobId"
} catch {
    Write-Error "Failed to create job: $_"
    exit 1
}

# 2. Test Lock (Concurrent)
Write-Host "Testing Lock (Concurrent Job)..."
try {
    $resp2 = Invoke-RestMethod -Uri $url -Method Post -Body $body -ContentType "application/json" -ErrorAction Stop
    Write-Error "ERROR: Should have failed with 409 Conflict, but got success"
    exit 1
} catch {
    if ($_.Exception.Response.StatusCode -eq [System.Net.HttpStatusCode]::Conflict) {
        Write-Host "SUCCESS: Lock enforced (409 Conflict)"
    } else {
        Write-Error "ERROR: Expected 409, got $($_.Exception.Response.StatusCode)"
        exit 1
    }
}

# 3. Poll Status
Write-Host "Polling Job Status..."
$status = "Pending"
while ($status -ne "Ready" -and $status -ne "Failed") {
    Start-Sleep -Seconds 1
    try {
        $j = Invoke-RestMethod -Uri "$url/$jobId" -Method Get
        $status = $j.phase
        Write-Host "Phase: $status (Epoch: $($j.progress.epoch), Step: $($j.progress.step))"
    } catch {
        Write-Error "Failed to poll job: $_"
        exit 1
    }
}

if ($status -eq "Failed") {
    Write-Error "Job Failed: $($j.error)"
    exit 1
}

Write-Host "Job Completed Successfully!"
exit 0
