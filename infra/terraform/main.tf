terraform {
  required_version = ">= 1.6"
  required_providers {
    lxd = {
      source  = "terraform-lxd/lxd"
      version = "~> 1.0"
    }
  }
}

provider "lxd" {
  generate_client_certificates = true
  accept_remote_certificate    = true
}

resource "lxd_network" "menzi" {
  name = "menzinet"
  type = "bridge"
  config = {
    "ipv4.address" = var.menzi_network_ipv4
    "ipv4.nat"     = "true"
    "ipv6.address" = "auto"
  }
}

resource "lxd_storage_pool" "menzi" {
  name   = var.storage_pool
  driver = "dir"
}

resource "lxd_profile" "menzi" {
  name        = "menzi"
  description = "Baseline profile for Menzi services"
  device {
    name = "root"
    type = "disk"
    properties = {
      path = "/"
      pool = lxd_storage_pool.menzi.name
    }
  }
  device {
    name = "eth0"
    type = "nic"
    properties = {
      name    = "eth0"
      network = lxd_network.menzi.name
    }
  }
}

resource "lxd_instance" "postgres" {
  name     = "menzi-postgres"
  image    = var.postgres_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}

resource "lxd_instance" "nats" {
  name     = "menzi-nats"
  image    = var.nats_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}

resource "lxd_instance" "core_api" {
  name     = "menzi-core-api"
  image    = var.ubuntu_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}

resource "lxd_instance" "llm_gateway" {
  name     = "menzi-llm-gateway"
  image    = var.ubuntu_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}

resource "lxd_instance" "session_proxy" {
  name     = "menzi-session-proxy"
  image    = var.ubuntu_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}

resource "lxd_instance" "orchestrator" {
  name     = "menzi-orchestrator"
  image    = var.ubuntu_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}

resource "lxd_instance" "supervisor" {
  name     = "menzi-supervisor"
  image    = var.ubuntu_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
  depends_on = [lxd_instance.orchestrator]
}

resource "lxd_instance" "previews" {
  count    = var.preview_count
  name     = "menzi-previews-${count.index + 1}"
  image    = var.ubuntu_image
  profiles = [lxd_profile.menzi.name]
  config = {
    "boot.autostart" = "true"
  }
}