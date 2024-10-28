#
# VPC
#

resource "aws_vpc" "main" {
  cidr_block = "10.0.0.0/16"

  tags = {
    Name = "udp-router-loadtest"
  }
}

resource "aws_subnet" "client" {
  vpc_id     = aws_vpc.main.id
  cidr_block = "10.0.1.0/24"
  availability_zone = "us-east-1a"

  tags = {
    Name = "udp-router-loadtest-client"
  }
}

resource "aws_subnet" "router" {
  vpc_id     = aws_vpc.main.id
  cidr_block = "10.0.2.0/24"
  availability_zone = "us-east-1a"

  tags = {
    Name = "udp-router-loadtest-router"
  }
}

resource "aws_subnet" "server" {
  vpc_id     = aws_vpc.main.id
  cidr_block = "10.0.3.0/24"
  availability_zone = "us-east-1a"

  tags = {
    Name = "udp-router-loadtest-server"
  }
}

resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id

  tags = {
    Name = "udp-router-loadtest-igw"
  }
}

resource "aws_route_table" "igw" {
 vpc_id = aws_vpc.main.id
 route {
   cidr_block = "0.0.0.0/0"
   gateway_id = aws_internet_gateway.main.id
 }

 tags = {
   Name = "udp-router-loadtest-igw"
 }
}

resource "aws_route_table_association" "client" {
 subnet_id      = aws_subnet.client.id
 route_table_id = aws_route_table.igw.id
}

resource "aws_route_table_association" "router" {
 subnet_id      = aws_subnet.router.id
 route_table_id = aws_route_table.igw.id
}

resource "aws_route_table_association" "server" {
 subnet_id      = aws_subnet.server.id
 route_table_id = aws_route_table.igw.id
}

#
# SSH key
#

resource "aws_key_pair" "instance" {
  key_name   = "udp-router-loadtest-key"
  public_key = var.public_key
}

#
# Security Group
#

resource "aws_security_group" "instance" {
  name        = "udp-router-loadtest-sg"
  vpc_id      = aws_vpc.main.id

  tags = {
    Terraform = "true"
    Name = "ed-testing"
  }
}

resource "aws_vpc_security_group_ingress_rule" "allow_local" {
  security_group_id = aws_security_group.instance.id
  cidr_ipv4         = "10.0.0.0/16"
  ip_protocol       = "-1" # semantically equivalent to all ports
}

resource "aws_vpc_security_group_ingress_rule" "allow_ssh" {
  security_group_id = aws_security_group.instance.id
  cidr_ipv4         = "0.0.0.0/0"
  from_port         = 22
  to_port           = 22
  ip_protocol       = "tcp"
}

resource "aws_vpc_security_group_egress_rule" "allow_all" {
  security_group_id = aws_security_group.instance.id
  cidr_ipv4         = "0.0.0.0/0"
  ip_protocol       = "-1"
}

#
# EC2 instances
#

module "clients" {
  source  = "terraform-aws-modules/ec2-instance/aws"

  count = var.num_client_instances
  name = "udp-router-loadtest-client-${count.index}"

  private_ip             = cidrhost("10.0.1.0/24", 10 + count.index)
  associate_public_ip_address = true
  ami                    = "ami-0929f698754f34ba7"
  instance_type          = "c7gn.xlarge"
  key_name               = "udp-router-loadtest-key"
  monitoring             = true
  vpc_security_group_ids = [aws_security_group.instance.id]
  subnet_id              = aws_subnet.client.id

  root_block_device = [{
    volume_size = 30
    volume_type = "gp3"
    encrypted   = false
  }]

  tags = {
    Terraform   = "true"
    Environment = "dev"
  }
}

module "router" {
  source  = "terraform-aws-modules/ec2-instance/aws"
  name = "udp-router-loadtest-router"

  private_ip             = "10.0.2.10"
  secondary_private_ips  = ["10.0.2.11"]
  associate_public_ip_address = true
  ami                    = "ami-0929f698754f34ba7"
  instance_type          = "c7gn.xlarge"
  key_name               = "udp-router-loadtest-key"
  monitoring             = true
  vpc_security_group_ids = [aws_security_group.instance.id]
  subnet_id              = aws_subnet.router.id

  root_block_device = [{
    volume_size = 30
    volume_type = "gp3"
    encrypted   = false
  }]

  tags = {
    Terraform   = "true"
    Environment = "dev"
  }
}

module "servers" {
  source  = "terraform-aws-modules/ec2-instance/aws"
  count = var.num_server_instances

  name = "udp-router-loadtest-server-${count.index}"

  private_ip             = cidrhost("10.0.3.0/24", 10 + count.index)
  associate_public_ip_address = true
  ami                    = "ami-0929f698754f34ba7"
  instance_type          = "c7gn.xlarge"
  key_name               = "udp-router-loadtest-key"
  monitoring             = true
  vpc_security_group_ids = [aws_security_group.instance.id]
  subnet_id              = aws_subnet.server.id

  root_block_device = [{
    volume_size = 30
    volume_type = "gp3"
    encrypted   = false
  }]

  tags = {
    Terraform   = "true"
    Environment = "dev"
  }
}
