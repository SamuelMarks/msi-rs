Vagrant.configure("2") do |config|
  config.vm.box = "bento/windows-11"
  config.vm.communicator = "winrm"
  config.winrm.transport = :plaintext
  config.winrm.basic_auth_only = true
  config.vm.network "forwarded_port", guest: 3389, host: 3389, id: "rdp", auto_correct: true, disabled: true
  config.vm.network "forwarded_port", guest: 5985, host: 5985, id: "winrm", auto_correct: true, disabled: true
  config.vm.network "forwarded_port", guest: 22, host: 50022, id: "ssh", auto_correct: true, disabled: true
end
