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
      
      private function updateValue(field:TextField, values:Array, bonuses:Array, row:int) : void
      {
         var valueText:String = values[row] != undefined ? values[row] : "";
         var bonus:int = bonuses[row] != undefined ? bonuses[row] : null;
         field.text = valueText + (bonus && bonus > 0 ? " (+" + bonus + "%)" : "");
      }
      
      override protected function invalidateData() : void
      {
         var columns:Array;
         trace("TVS::invalidateData",_data);
         if(!_data)
         {
            return;
         }
         this.columnValue.visible = _data.showValues != undefined ? _data.showValues : true;
         if(!this.columnValue.visible)
         {
            shift = 93;
            this.columnMoney.x -= shift;
            this.columnExperience.x -= shift;
            this.columnReputation.x -= shift;
            this.background.width -= shift;
         }
         columns = _data.columnsData as Array;
         trace("TVS::invalidateData",columns);
         var rowCount:int = 1;
         var i:int = 0;
         var shift:int;
         var column:Object;
         var renderer:MovieClip;
         var values:Array;
         var bonuses:Array;
         while(i < columns.length)
         {
            column = columns[i];
            renderer = this.columnRenderers[i];
            trace("TVS::columns",renderer,column);
            if(column.values is Array)
            {
               values = column.values as Array;
            }
            else
            {
               values = [column.values];
            }
            if(column.bonus is Array)
            {
               bonuses = column.bonus as Array;
            }
            else
            {
               bonuses = [column.bonus];
            }
            renderer.textFieldLabel.text = column.label;
            renderer.textFieldRow2.visible = values.length > 1;
            renderer.textFieldRow3.visible = values.length > 2;
            rowCount = Math.max(rowCount,values.length);
            this.updateValue(renderer.textFieldRow1,values,bonuses,0);
            this.updateValue(renderer.textFieldRow2,values,bonuses,1);
            this.updateValue(renderer.textFieldRow3,values,bonuses,2);
            i++;
         }
         this.background.height = 140 - 24 * 3 + 24 * rowCount;
      }
   }
}

