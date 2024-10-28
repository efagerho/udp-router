module "vpc" {
  source = "terraform-aws-modules/vpc/aws"

  name = "udp-router-loadtest"
  cidr = "10.0.0.0/16"

  azs             = ["us-east-1a"]
  public_subnets  = ["10.0.0.0/24"]
  private_subnets  = ["10.0.1.0/24"]

  enable_nat_gateway = true
  enable_vpn_gateway = true

  tags = {
    Terraform = "true"
    Environment = "ed-testing"
  }
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
  vpc_id      = module.vpc.vpc_id

  tags = {
    Terraform = "true"
    Name = "ed-testing"
  }
}

resource "aws_vpc_security_group_ingress_rule" "allow_local" {
  security_group_id = aws_security_group.instance.id
  cidr_ipv4         = module.vpc.vpc_cidr_block
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

  private_ip             = cidrhost("10.0.0.0/24", 10 + count.index)
  associate_public_ip_address = true
  ami                    = "ami-0929f698754f34ba7"
  instance_type          = "c7gn.xlarge"
  key_name               = "udp-router-loadtest-key"
  monitoring             = true
  vpc_security_group_ids = [aws_security_group.instance.id]
  subnet_id              = module.vpc.public_subnets[0]

  tags = {
    Terraform   = "true"
    Environment = "dev"
  }
}

module "router" {
  source  = "terraform-aws-modules/ec2-instance/aws"
  name = "udp-router-loadtest-router"

  private_ip             = "10.0.0.100"
  secondary_private_ips  = ["10.0.0.101"]
  associate_public_ip_address = true
  ami                    = "ami-0929f698754f34ba7"
  instance_type          = "c7gn.xlarge"
  key_name               = "udp-router-loadtest-key"
  monitoring             = true
  vpc_security_group_ids = [aws_security_group.instance.id]
  subnet_id              = module.vpc.public_subnets[0]

  tags = {
    Terraform   = "true"
    Environment = "dev"
  }
}

module "servers" {
  source  = "terraform-aws-modules/ec2-instance/aws"
  count = var.num_server_instances

  name = "udp-router-loadtest-server-${count.index}"

  private_ip             = cidrhost("10.0.0.0/24", 200 + count.index)
  associate_public_ip_address = true
  ami                    = "ami-0929f698754f34ba7"
  instance_type          = "c7gn.xlarge"
  key_name               = "udp-router-loadtest-key"
  monitoring             = true
  vpc_security_group_ids = [aws_security_group.instance.id]
  subnet_id              = module.vpc.public_subnets[0]

  tags = {
    Terraform   = "true"
    Environment = "dev"
  }
}
