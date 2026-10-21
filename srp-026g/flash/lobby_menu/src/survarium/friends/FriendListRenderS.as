package survarium.friends
{
   import flash.display.MovieClip;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.controls.FixedTextDots;
   
   public class FriendListRenderS extends ListItemRenderer
   {
      
      public var nameTextfield:FixedTextDots;
      
      public var statusIndicator:MovieClip;
      
      public function FriendListRenderS()
      {
         super();
      }
      
      override public function setData(newData:Object) : void
      {
         this.data = newData;
         invalidateData();
      }
      
      override protected function updateText() : void
      {
         if(!data)
         {
            return;
         }
         trace("FriendListRenderS::",this.nameTextfield,this.statusIndicator);
         this.nameTextfield.text = data.abbr ? "[" + data.abbr + "] " + data.name : data.name;
         if(_data.ignored)
         {
            this.statusIndicator.gotoAndStop("offline");
         }
         else if(_data.status == 0)
         {
            this.statusIndicator.gotoAndStop("online");
         }
         else if(_data.status == 1)
         {
            this.statusIndicator.gotoAndStop("ingame");
         }
         else
         {
            this.statusIndicator.gotoAndStop("offline");
         }
      }
   }
}

