package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import survarium.Util;
   
   public class LabelCost extends MovieClip
   {
      
      public var icon:MovieClip;
      
      public var costBack:MovieClip;
      
      public var textField:TextField;
      
      public var moveIcon:Boolean = true;
      
      public function LabelCost()
      {
         super();
         // Always silver, the icon's and back's first frame: there is no gold (the owner, 2026-11-08).
         this.icon.gotoAndStop(1);
         if(this.costBack)
         {
            this.costBack.gotoAndStop(1);
         }
         else
         {
            moveIcon = false;
         }
      }
      
      public function set cost(value:uint) : void
      {
         var costText:String = Util.getMoneyString(value);
         if(this.textField.text != costText)
         {
            this.textField.text = costText;
         }
         if(this.moveIcon)
         {
            this.icon.x = this.textField.x + (this.textField.width - this.textField.textWidth) - this.icon.width - 5;
         }
      }
   }
}

