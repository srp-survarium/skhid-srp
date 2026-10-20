package survarium.buttons
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   
   public class ButtonFriends extends ButtonClick
   {
      
      public var invitesIcon:MovieClip;
      
      public var background:MovieClip;
      
      public var icon:Sprite;
      
      protected var _invites:uint = 0;
      
      public function ButtonFriends()
      {
         super();
      }
      
      public function set invites(value:uint) : void
      {
         _invites = value;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         constraintsDisabled = true;
      }
      
      override protected function updateText() : void
      {
         var totalW:int;
         super.updateText();
         if(this._invites != 0)
         {
            this.invitesIcon.visible = true;
            totalW = this.icon.width + 3 + textField.textWidth + 5 + this.invitesIcon.width;
            this.icon.x = (134 - totalW) / 2;
            textField.x = this.icon.x + this.icon.width + 3;
            this.invitesIcon.x = textField.x + textField.textWidth + 5;
            if(this.invitesIcon.textField.text != this._invites.toString())
            {
               this.invitesIcon.textField.text = this._invites.toString();
            }
         }
         else
         {
            this.invitesIcon.visible = false;
            totalW = this.icon.width + 3 + textField.textWidth;
            this.icon.x = (134 - totalW) / 2;
            textField.x = this.icon.x + this.icon.width + 3;
            this.invitesIcon.x = this.icon.x;
         }
         this.icon.y = textField.y + 5;
         this.invitesIcon.y = this.icon.y - 6;
         trace("BF:updated",width,"invites::",this._invites);
      }
   }
}

