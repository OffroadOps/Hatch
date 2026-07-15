namespace Hatch.Forms
{
    partial class AboutForm
    {
        private System.ComponentModel.IContainer components = null;

        protected override void Dispose(bool disposing)
        {
            if (disposing && (components != null))
            {
                components.Dispose();
            }
            base.Dispose(disposing);
        }

        private void InitializeComponent()
        {
            this.LogoPictureBox = new System.Windows.Forms.PictureBox();
            this.AppVersionLabel = new System.Windows.Forms.Label();
            this.XrayVersionLabel = new System.Windows.Forms.Label();
            this.SingBoxVersionLabel = new System.Windows.Forms.Label();
            this.DescriptionLabel = new System.Windows.Forms.Label();
            this.TributeLabel = new System.Windows.Forms.Label();
            this.NetchLinkLabel = new System.Windows.Forms.LinkLabel();
            this.GitHubLinkLabel = new System.Windows.Forms.LinkLabel();
            this.CheckUpdateButton = new System.Windows.Forms.Button();
            this.UpdateCoresButton = new System.Windows.Forms.Button();
            this.CloseButton = new System.Windows.Forms.Button();
            ((System.ComponentModel.ISupportInitialize)(this.LogoPictureBox)).BeginInit();
            this.SuspendLayout();
            //
            // LogoPictureBox
            //
            this.LogoPictureBox.Location = new System.Drawing.Point(20, 20);
            this.LogoPictureBox.Name = "LogoPictureBox";
            this.LogoPictureBox.Size = new System.Drawing.Size(64, 64);
            this.LogoPictureBox.TabIndex = 0;
            this.LogoPictureBox.TabStop = false;
            //
            // AppVersionLabel
            //
            this.AppVersionLabel.AutoSize = true;
            this.AppVersionLabel.Font = new System.Drawing.Font("Segoe UI", 16F, System.Drawing.FontStyle.Bold);
            this.AppVersionLabel.Location = new System.Drawing.Point(100, 35);
            this.AppVersionLabel.Name = "AppVersionLabel";
            this.AppVersionLabel.Size = new System.Drawing.Size(150, 30);
            this.AppVersionLabel.TabIndex = 1;
            this.AppVersionLabel.Text = "Hatch v2.0.0";
            //
            // XrayVersionLabel
            //
            this.XrayVersionLabel.AutoSize = true;
            this.XrayVersionLabel.Font = new System.Drawing.Font("Segoe UI", 10F);
            this.XrayVersionLabel.Location = new System.Drawing.Point(20, 95);
            this.XrayVersionLabel.Name = "XrayVersionLabel";
            this.XrayVersionLabel.Size = new System.Drawing.Size(150, 19);
            this.XrayVersionLabel.TabIndex = 2;
            this.XrayVersionLabel.Text = "Xray-core: Checking...";
            //
            // SingBoxVersionLabel
            //
            this.SingBoxVersionLabel.AutoSize = true;
            this.SingBoxVersionLabel.Font = new System.Drawing.Font("Segoe UI", 10F);
            this.SingBoxVersionLabel.Location = new System.Drawing.Point(20, 120);
            this.SingBoxVersionLabel.Name = "SingBoxVersionLabel";
            this.SingBoxVersionLabel.Size = new System.Drawing.Size(150, 19);
            this.SingBoxVersionLabel.TabIndex = 3;
            this.SingBoxVersionLabel.Text = "sing-box: Checking...";
            //
            // DescriptionLabel
            //
            this.DescriptionLabel.AutoSize = true;
            this.DescriptionLabel.Font = new System.Drawing.Font("Segoe UI", 9F);
            this.DescriptionLabel.ForeColor = System.Drawing.Color.Gray;
            this.DescriptionLabel.Location = new System.Drawing.Point(20, 155);
            this.DescriptionLabel.Name = "DescriptionLabel";
            this.DescriptionLabel.Size = new System.Drawing.Size(380, 15);
            this.DescriptionLabel.TabIndex = 4;
            this.DescriptionLabel.Text = "A lightweight and powerful network proxy tool for Windows";
            //
            // TributeLabel
            //
            this.TributeLabel.AutoSize = true;
            this.TributeLabel.Font = new System.Drawing.Font("Segoe UI", 9F, System.Drawing.FontStyle.Italic);
            this.TributeLabel.ForeColor = System.Drawing.Color.Gray;
            this.TributeLabel.Location = new System.Drawing.Point(20, 180);
            this.TributeLabel.Name = "TributeLabel";
            this.TributeLabel.Size = new System.Drawing.Size(280, 15);
            this.TributeLabel.TabIndex = 5;
            this.TributeLabel.Text = "🥚 Hatched from Netch - Without Netch, no Hatch";
            //
            // NetchLinkLabel
            //
            this.NetchLinkLabel.AutoSize = true;
            this.NetchLinkLabel.Font = new System.Drawing.Font("Segoe UI", 9F);
            this.NetchLinkLabel.Location = new System.Drawing.Point(20, 205);
            this.NetchLinkLabel.Name = "NetchLinkLabel";
            this.NetchLinkLabel.Size = new System.Drawing.Size(180, 15);
            this.NetchLinkLabel.TabIndex = 6;
            this.NetchLinkLabel.TabStop = true;
            this.NetchLinkLabel.Text = "Original Project: Netch on GitHub";
            this.NetchLinkLabel.LinkClicked += new System.Windows.Forms.LinkLabelLinkClickedEventHandler(this.NetchLinkLabel_LinkClicked);
            //
            // GitHubLinkLabel
            //
            this.GitHubLinkLabel.AutoSize = true;
            this.GitHubLinkLabel.Font = new System.Drawing.Font("Segoe UI", 9F);
            this.GitHubLinkLabel.Location = new System.Drawing.Point(20, 230);
            this.GitHubLinkLabel.Name = "GitHubLinkLabel";
            this.GitHubLinkLabel.Size = new System.Drawing.Size(150, 15);
            this.GitHubLinkLabel.TabIndex = 7;
            this.GitHubLinkLabel.TabStop = true;
            this.GitHubLinkLabel.Text = "Visit Hatch on GitHub";
            this.GitHubLinkLabel.LinkClicked += new System.Windows.Forms.LinkLabelLinkClickedEventHandler(this.GitHubLinkLabel_LinkClicked);
            //
            // CheckUpdateButton
            //
            this.CheckUpdateButton.Location = new System.Drawing.Point(20, 265);
            this.CheckUpdateButton.Name = "CheckUpdateButton";
            this.CheckUpdateButton.Size = new System.Drawing.Size(150, 35);
            this.CheckUpdateButton.TabIndex = 8;
            this.CheckUpdateButton.Text = "Check for Updates";
            this.CheckUpdateButton.UseVisualStyleBackColor = true;
            this.CheckUpdateButton.Click += new System.EventHandler(this.CheckUpdateButton_Click);
            //
            // UpdateCoresButton
            //
            this.UpdateCoresButton.Location = new System.Drawing.Point(180, 265);
            this.UpdateCoresButton.Name = "UpdateCoresButton";
            this.UpdateCoresButton.Size = new System.Drawing.Size(150, 35);
            this.UpdateCoresButton.TabIndex = 9;
            this.UpdateCoresButton.Text = "Update Cores";
            this.UpdateCoresButton.UseVisualStyleBackColor = true;
            this.UpdateCoresButton.Click += new System.EventHandler(this.UpdateCoresButton_Click);
            //
            // CloseButton
            //
            this.CloseButton.Location = new System.Drawing.Point(340, 265);
            this.CloseButton.Name = "CloseButton";
            this.CloseButton.Size = new System.Drawing.Size(100, 35);
            this.CloseButton.TabIndex = 10;
            this.CloseButton.Text = "Close";
            this.CloseButton.UseVisualStyleBackColor = true;
            this.CloseButton.Click += new System.EventHandler((s, e) => this.Close());
            //
            // AboutForm
            //
            this.AutoScaleDimensions = new System.Drawing.SizeF(7F, 15F);
            this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font;
            this.ClientSize = new System.Drawing.Size(460, 315);
            this.Controls.Add(this.CloseButton);
            this.Controls.Add(this.UpdateCoresButton);
            this.Controls.Add(this.CheckUpdateButton);
            this.Controls.Add(this.GitHubLinkLabel);
            this.Controls.Add(this.NetchLinkLabel);
            this.Controls.Add(this.TributeLabel);
            this.Controls.Add(this.DescriptionLabel);
            this.Controls.Add(this.SingBoxVersionLabel);
            this.Controls.Add(this.XrayVersionLabel);
            this.Controls.Add(this.AppVersionLabel);
            this.Controls.Add(this.LogoPictureBox);
            this.FormBorderStyle = System.Windows.Forms.FormBorderStyle.FixedDialog;
            this.MaximizeBox = false;
            this.MinimizeBox = false;
            this.Name = "AboutForm";
            this.StartPosition = System.Windows.Forms.FormStartPosition.CenterParent;
            this.Text = "About Hatch";
            ((System.ComponentModel.ISupportInitialize)(this.LogoPictureBox)).EndInit();
            this.ResumeLayout(false);
            this.PerformLayout();
        }

        private System.Windows.Forms.PictureBox LogoPictureBox;
        private System.Windows.Forms.Label AppVersionLabel;
        private System.Windows.Forms.Label XrayVersionLabel;
        private System.Windows.Forms.Label SingBoxVersionLabel;
        private System.Windows.Forms.Label DescriptionLabel;
        private System.Windows.Forms.Label TributeLabel;
        private System.Windows.Forms.LinkLabel NetchLinkLabel;
        private System.Windows.Forms.LinkLabel GitHubLinkLabel;
        private System.Windows.Forms.Button CheckUpdateButton;
        private System.Windows.Forms.Button UpdateCoresButton;
        private System.Windows.Forms.Button CloseButton;
    }
}
