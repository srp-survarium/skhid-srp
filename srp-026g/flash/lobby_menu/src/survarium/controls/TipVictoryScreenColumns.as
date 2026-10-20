package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.TipRenderer;
   
   public class TipVictoryScreenColumns extends TipRenderer
   {
      
      public var columnProfile:MovieClip;
      
      public var columnPlayTime:MovieClip;
      
      public var columnValue:MovieClip;
      
      public var columnMoney:MovieClip;
      
      public var columnExperience:MovieClip;
      
      public var columnReputation:MovieClip;
      
      public var background:MovieClip;
      
      private var columnRenderers:Array = [];
      
      public function TipVictoryScreenColumns()
      {
         super();
         this.columnRenderers.push(this.columnProfile,this.columnPlayTime,this.columnValue,this.columnMoney,this.columnExperience,this.columnReputation);
      }
      
      private function updateValue(param1:TextField, param2:Array, param3:Array, param4:int) : void
      {
         var _loc5_:String = param2[param4] != undefined ? param2[param4] : "";
         var _loc6_:int = param3[param4] != undefined ? param3[param4] : null;
         param1.text = _loc5_ + (_loc6_ && _loc6_ > 0 ? " (+" + _loc6_ + "%)" : "");
      }
      
      override protected function invalidateData() : void
      {
         var _loc1_:Array;
         trace("TVS::invalidateData",_data);
         if(!_data)
         {
            return;
         }
         this.columnValue.visible = _data.showValues != undefined ? _data.showValues : true;
         if(!this.columnValue.visible)
         {
            _loc4_ = 93;
            this.columnMoney.x -= _loc4_;
            this.columnExperience.x -= _loc4_;
            this.columnReputation.x -= _loc4_;
            this.background.width -= _loc4_;
         }
         _loc1_ = _data.columnsData as Array;
         trace("TVS::invalidateData",_loc1_);
         var _loc2_:int = 1;
         var _loc3_:int = 0;
         var _loc4_:int;
         var _loc5_:Object;
         var _loc6_:MovieClip;
         var _loc7_:Array;
         var _loc8_:Array;
         while(_loc3_ < _loc1_.length)
         {
            _loc5_ = _loc1_[_loc3_];
            _loc6_ = this.columnRenderers[_loc3_];
            trace("TVS::columns",_loc6_,_loc5_);
            if(_loc5_.values is Array)
            {
               _loc7_ = _loc5_.values as Array;
            }
            else
            {
               _loc7_ = [_loc5_.values];
            }
            if(_loc5_.bonus is Array)
            {
               _loc8_ = _loc5_.bonus as Array;
            }
            else
            {
               _loc8_ = [_loc5_.bonus];
            }
            _loc6_.textFieldLabel.text = _loc5_.label;
            _loc6_.textFieldRow2.visible = _loc7_.length > 1;
            _loc6_.textFieldRow3.visible = _loc7_.length > 2;
            _loc2_ = Math.max(_loc2_,_loc7_.length);
            this.updateValue(_loc6_.textFieldRow1,_loc7_,_loc8_,0);
            this.updateValue(_loc6_.textFieldRow2,_loc7_,_loc8_,1);
            this.updateValue(_loc6_.textFieldRow3,_loc7_,_loc8_,2);
            _loc3_++;
         }
         this.background.height = 140 - 24 * 3 + 24 * _loc2_;
      }
   }
}

