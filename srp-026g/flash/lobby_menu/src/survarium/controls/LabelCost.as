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
      
      protected var _moneyType:uint = 0;
      
      public function LabelCost()
      {
         super();
         if(!this.costBack)
         {
            moveIcon = false;
         }
      }
      
      public function set moneyType(value:uint) : void
      {
         _moneyType = value;
         this.icon.gotoAndStop(value ? 2 : 1);
         if(this.costBack)
         {
            this.costBack.gotoAndStop(value ? 2 : 1);
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
            this.icon.x = this.textField.x + (this.textField.width - this.textField.textWidth) - this.icon.width - 5 + (this._moneyType ? 14 : 0);
         }
      }
   }
}

