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
      
      public function set moneyType(param1:uint) : void
      {
         _moneyType = param1;
         this.icon.gotoAndStop(param1 ? 2 : 1);
         if(this.costBack)
         {
            this.costBack.gotoAndStop(param1 ? 2 : 1);
         }
      }
      
      public function set cost(param1:uint) : void
      {
         var _loc2_:String = Util.getMoneyString(param1);
         if(this.textField.text != _loc2_)
         {
            this.textField.text = _loc2_;
         }
         if(this.moveIcon)
         {
            this.icon.x = this.textField.x + (this.textField.width - this.textField.textWidth) - this.icon.width - 5 + (this._moneyType ? 14 : 0);
         }
      }
   }
}

