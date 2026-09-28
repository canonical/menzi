variable "menzi_network_ipv4" {
  description = "IPv4 subnet for the Menzi bridge network"
  type        = string
  default     = "10.120.0.1/24"
}

variable "storage_pool" {
  description = "LXD storage pool for Menzi instances"
  type        = string
  default     = "menzi"
}

variable "ubuntu_image" {
  description = "LXD image alias for service hosts"
  type        = string
  default     = "ubuntu/24.04"
}

variable "postgres_image" {
  description = "LXD image alias running PostgreSQL"
  type        = string
  default     = "ubuntu/24.04"
}

variable "nats_image" {
  description = "LXD image alias running NATS Server"
  type        = string
  default     = "ubuntu/24.04"
}

variable "preview_count" {
  description = "Number of preview service replicas"
  type        = number
  default     = 1
}