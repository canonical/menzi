output "network_name" {
  description = "Bridge network shared by Menzi instances"
  value       = lxd_network.menzi.name
}

output "storage_pool" {
  description = "Storage pool backing Menzi instances"
  value       = lxd_storage_pool.menzi.name
}

output "core_api_instance" {
  description = "Name of the control plane instance"
  value       = lxd_instance.core_api.name
}

output "orchestrator_instance" {
  description = "Name of the orchestrator instance"
  value       = lxd_instance.orchestrator.name
}

output "preview_instances" {
  description = "Names of the preview service instances"
  value       = lxd_instance.previews[*].name
}

output "workspace_instance" {
  description = "Name of the workspace service instance"
  value       = lxd_instance.workspace.name
}