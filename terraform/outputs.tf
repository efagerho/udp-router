output "client_instance_ips" {
  value = module.clients[*].public_ip
}

output "router_instance_ip" {
  value = module.router.public_ip
}

output "server_instance_ip" {
  value = module.servers[*].public_ip
}
